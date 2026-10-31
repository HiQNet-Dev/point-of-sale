//! Snapshots consistentes y restauración a archivo nuevo para mantenimiento.
//!
//! No es un comando IPC ni permite restaurar encima de una instalación. El
//! entrypoint autorizado debe validar directorios/ACL y coordinar el candado.
//! `VACUUM INTO` captura SQLite, incluido WAL, sin copiar el archivo abierto.
use sha2::{Digest, Sha256};
use sqlx::{
    ConnectOptions, Connection, SqliteConnection, SqlitePool, sqlite::SqliteConnectOptions,
};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Errores sin rutas, SQL ni datos de la instalación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BackupError {
    /// La ruta debe ser absoluta y representar un archivo nuevo.
    #[error("destino de mantenimiento inválido")]
    InvalidDestination,
    /// Nunca se reemplaza un archivo existente, ni siquiera vacío.
    #[error("el destino ya existe")]
    DestinationExists,
    /// E/S o motor SQLite no disponible.
    #[error("operación de mantenimiento no disponible")]
    Unavailable,
    /// Falló integridad, FK, historial/checksum o digest del snapshot.
    #[error("snapshot rechazado por integridad o compatibilidad")]
    Rejected,
}

/// Snapshot cerrado y verificado. No es serializable ni expone rutas por Debug.
pub struct VerifiedBackup {
    path: PathBuf,
    digest: [u8; 32],
}
impl std::fmt::Debug for VerifiedBackup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedBackup(REDACTED)")
    }
}
impl VerifiedBackup {
    /// SHA-256 del archivo cerrado; no incluye datos o secretos en claro.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

fn reject_sidecars(path: &Path) -> Result<(), BackupError> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        match std::fs::symlink_metadata(Path::new(&name)) {
            Ok(_) => return Err(BackupError::DestinationExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(BackupError::Unavailable),
        }
    }
    Ok(())
}

fn reserve(path: &Path) -> Result<File, BackupError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(BackupError::InvalidDestination);
    }
    reject_sidecars(path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            BackupError::DestinationExists
        } else {
            BackupError::Unavailable
        }
    })
}

fn digest_file(mut file: File) -> Result<[u8; 32], BackupError> {
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16_384];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| BackupError::Unavailable)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().into())
}

async fn validate(path: &Path) -> Result<(), BackupError> {
    reject_sidecars(path).map_err(|_| BackupError::Rejected)?;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false)
        .disable_statement_logging();
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|_| BackupError::Rejected)?;
    let result = validate_connection(&mut connection).await;
    connection
        .close()
        .await
        .map_err(|_| BackupError::Unavailable)?;
    result
}
async fn validate_connection(connection: &mut SqliteConnection) -> Result<(), BackupError> {
    let integrity: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(&mut *connection)
        .await
        .map_err(|_| BackupError::Rejected)?;
    if integrity != ["ok"] {
        return Err(BackupError::Rejected);
    }
    if sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| BackupError::Rejected)?
        .is_some()
    {
        return Err(BackupError::Rejected);
    }
    let dirty: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success <> 1")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| BackupError::Rejected)?;
    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(connection)
            .await
            .map_err(|_| BackupError::Rejected)?;
    let migrator = sqlx::migrate!("../../migrations/local");
    if dirty != 0
        || applied.len() != migrator.iter().count()
        || applied
            .iter()
            .zip(migrator.iter())
            .any(|((version, checksum), expected)| {
                *version != expected.version || checksum.as_slice() != expected.checksum.as_ref()
            })
    {
        return Err(BackupError::Rejected);
    }
    Ok(())
}

/// Genera y verifica una instantánea consistente con historial aprobado.
///
/// # Errors
/// No sobrescribe ni elimina archivos. Ante un fallo puede quedar un artefacto
/// parcial reservado: no se publica VerifiedBackup ni se activa como instalación.
/// La comprobación de historial no certifica el esquema de negocio completo.
pub async fn create_backup(
    pool: &SqlitePool,
    destination: &Path,
) -> Result<VerifiedBackup, BackupError> {
    let path = destination.to_path_buf();
    let reserve_path = path.clone();
    let file = tokio::task::spawn_blocking(move || reserve(&reserve_path))
        .await
        .map_err(|_| BackupError::Unavailable)??;
    let name = path.to_str().ok_or(BackupError::InvalidDestination)?;
    sqlx::query("VACUUM INTO ?")
        .bind(name)
        .execute(pool)
        .await
        .map_err(|_| BackupError::Unavailable)?;
    let digest = tokio::task::spawn_blocking(move || {
        file.sync_all().map_err(|_| BackupError::Unavailable)?;
        digest_file(file)
    })
    .await
    .map_err(|_| BackupError::Unavailable)??;
    validate(&path).await?;
    Ok(VerifiedBackup { path, digest })
}

/// Reabre un snapshot cerrado tras reiniciar la herramienta administrativa.
///
/// El digest debe venir del manifiesto custodiado por el administrador, no
/// calcularse y aceptarse ciegamente desde el mismo archivo no confiable. El
/// digest aporta integridad, no autorización de recuperación ni firma digital.
///
/// # Errors
/// Rechaza snapshots modificados, inválidos o con historial incompatible.
pub async fn verify_existing_backup(
    path: &Path,
    expected_digest: [u8; 32],
) -> Result<VerifiedBackup, BackupError> {
    validate(path).await?;
    let source = path.to_path_buf();
    let digest = tokio::task::spawn_blocking(move || {
        digest_file(File::open(&source).map_err(|_| BackupError::Unavailable)?)
    })
    .await
    .map_err(|_| BackupError::Unavailable)??;
    if digest != expected_digest {
        return Err(BackupError::Rejected);
    }
    Ok(VerifiedBackup {
        path: path.to_path_buf(),
        digest,
    })
}

/// Restaura exclusivamente a un destino nuevo y verifica digest, integridad e historial.
///
/// # Errors
/// Rechaza artefactos modificados y destinos existentes. No sustituye la política
/// administrativa ni realiza cutover; nunca borra datos para intentar recuperar.
pub async fn restore_to_new_file(
    backup: &VerifiedBackup,
    destination: &Path,
) -> Result<(), BackupError> {
    validate(&backup.path).await?;
    let source = backup.path.clone();
    let target = destination.to_path_buf();
    let expected = backup.digest;
    tokio::task::spawn_blocking(move || {
        let input = File::open(&source).map_err(|_| BackupError::Unavailable)?;
        if digest_file(input)? != expected {
            return Err(BackupError::Rejected);
        }
        let mut input = File::open(&source).map_err(|_| BackupError::Unavailable)?;
        let mut output = reserve(&target)?;
        std::io::copy(&mut input, &mut output).map_err(|_| BackupError::Unavailable)?;
        output.flush().map_err(|_| BackupError::Unavailable)?;
        output.sync_all().map_err(|_| BackupError::Unavailable)?;
        if digest_file(File::open(&target).map_err(|_| BackupError::Unavailable)?)? != expected {
            return Err(BackupError::Rejected);
        }
        Ok(())
    })
    .await
    .map_err(|_| BackupError::Unavailable)??;
    validate(destination).await
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
