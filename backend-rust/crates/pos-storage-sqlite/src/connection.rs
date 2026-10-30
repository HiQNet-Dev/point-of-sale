//! Conexión SQLite Local: apertura validada, migraciones y checkpoint.
//!
//! Solo aplica el root embebido `migrations/local`; el esquema candidato sigue
//! fuera de este cargador.

use std::path::Path;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{ConnectOptions, SqlitePool};

/// Fallos de infraestructura sin SQL, rutas ni valores de filas en su representación pública.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// La ruta debe designar un archivo y el timeout debe ser explícito y acotado.
    #[error("configuración SQLite inválida")]
    InvalidConfiguration,
    /// No se pudo abrir o utilizar la conexión; no se imprime el error SQL original.
    #[error("operación SQLite fallida")]
    Database,
    /// La conexión no cumple los PRAGMA exigidos para la foundation.
    #[error("configuración efectiva SQLite insegura")]
    UnsafeConnection,
    /// SQLx rechazó la migración, incluido checksum o transacción fallida.
    #[error("migración SQLite rechazada")]
    Migration,
    /// El checkpoint no pudo completarse; el cierre no debe ocultar este resultado.
    #[error("checkpoint SQLite ocupado o incompleto")]
    Checkpoint,
}

/// Abre un archivo local con un único slot de escritura por proceso.
///
/// El timeout es una entrada explícita de laboratorio, no una política medida de producto.
/// No usar sobre NFS/SMB. Un pool de tamaño uno NO establece ownership entre procesos.
pub async fn open(path: &Path, busy_timeout: Duration) -> Result<SqlitePool, StorageError> {
    let name = path.to_str().ok_or(StorageError::InvalidConfiguration)?;
    if name.trim().is_empty()
        || has_disallowed_path_colon(name)
        || has_disallowed_path_question_mark(name)
        || name.contains(['#', '\0'])
        || path.is_dir()
        || busy_timeout < Duration::from_millis(1)
        || busy_timeout > Duration::from_secs(60)
    {
        return Err(StorageError::InvalidConfiguration);
    }
    let expected_timeout =
        i64::try_from(busy_timeout.as_millis()).map_err(|_| StorageError::InvalidConfiguration)?;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(busy_timeout)
        .disable_statement_logging();
    SqlitePoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            Box::pin(async move {
                let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                    .fetch_one(&mut *connection)
                    .await?;
                let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
                    .fetch_one(&mut *connection)
                    .await?;
                let timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
                    .fetch_one(&mut *connection)
                    .await?;
                let durability: i64 = sqlx::query_scalar("PRAGMA synchronous")
                    .fetch_one(&mut *connection)
                    .await?;
                if fk != 1 || journal != "wal" || timeout != expected_timeout || durability != 2 {
                    return Err(sqlx::Error::Protocol(
                        "SQLite PRAGMA verification failed".into(),
                    ));
                }
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .map_err(|_| StorageError::Database)
}

fn has_disallowed_path_colon(value: &str) -> bool {
    if cfg!(windows) {
        let bytes = value.as_bytes();
        let drive_index = if bytes.starts_with(b"\\\\?\\") { 5 } else { 1 };
        bytes.iter().enumerate().any(|(index, byte)| {
            *byte == b':'
                && !(index == drive_index
                    && bytes
                        .get(drive_index.saturating_sub(1))
                        .is_some_and(u8::is_ascii_alphabetic))
        })
    } else {
        value.contains(':')
    }
}

fn has_disallowed_path_question_mark(value: &str) -> bool {
    if cfg!(windows) && value.as_bytes().starts_with(b"\\\\?\\") {
        value.as_bytes()[3..].contains(&b'?')
    } else {
        value.contains('?')
    }
}

/// Aplica exclusivamente el root Local embebido; el esquema sigue siendo parcial.
///
/// SQLx conserva su propio historial y rechaza checksums alterados. Nunca se modifica Flyway.
pub async fn migrate(pool: &SqlitePool) -> Result<(), StorageError> {
    sqlx::migrate!("../../migrations/local")
        .run(pool)
        .await
        .map_err(|_| StorageError::Migration)
}

/// Solicita checkpoint después de drenar operaciones; no constituye un backup.
pub async fn checkpoint(pool: &SqlitePool) -> Result<(), StorageError> {
    let (busy, _, _): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(pool)
        .await
        .map_err(|_| StorageError::Database)?;
    if busy != 0 {
        return Err(StorageError::Checkpoint);
    }
    Ok(())
}
