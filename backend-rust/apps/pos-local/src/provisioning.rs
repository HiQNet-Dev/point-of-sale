//! Emisor nativo de autorizaciones de provisioning Free Local (candidato).
//!
//! El proceso administrativo recibe un directorio privado, genera el
//! identificador de instalación y el token con CSPRNG, persiste la autorización y
//! entrega el material por un archivo exclusivo. El token nunca viaja por
//! argumentos, stdout, logs ni variables públicas, y el archivo completo se trata
//! como material sensible.
//!
//! En Windows la entrega falla cerrada: la verificación efectiva de propietario y
//! DACL requiere un componente nativo aislado que todavía no existe.

use std::{fmt, fs, path::PathBuf, time::Duration, time::SystemTime, time::UNIX_EPOCH};

use pos_core::{
    identity::{ProvisioningAuthorization, ProvisioningPurpose},
    organization_branch::EntityId,
};
use pos_credentials::{ProvisioningToken, random_uuid_v4};
use pos_storage_sqlite::{open, provisioning::SqliteProvisioningStore};

/// Nombre del archivo de base que el directorio administrativo debe contener.
pub const DATABASE_FILE: &str = "hiqnet-pos.sqlite";
/// Nombre del archivo administrativo; contiene token e identificadores.
pub const AUTHORIZATION_FILE: &str = "provisioning-authorization.txt";
/// Vigencia por defecto de una autorización emitida, en segundos.
pub const DEFAULT_TTL_SECONDS: i64 = 600;

/// Operación administrativa solicitada.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvisioningCommand {
    /// Emite una autorización y entrega el token por archivo.
    Issue {
        /// Directorio privado de la instalación.
        directory: PathBuf,
        /// Vigencia solicitada, en segundos.
        ttl_seconds: i64,
    },
    /// Consulta el estado no sensible de la autorización entregada.
    Status {
        /// Directorio privado de la instalación.
        directory: PathBuf,
    },
    /// Revoca la autorización entregada.
    Revoke {
        /// Directorio privado de la instalación.
        directory: PathBuf,
    },
}

/// Fallo del emisor administrativo, sin material sensible en el mensaje.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningError {
    /// Argumentos incompletos o desconocidos.
    Usage,
    /// La plataforma no permite verificar la custodia del archivo.
    UnsupportedPlatform,
    /// El directorio no es privado, es un enlace o pertenece a otro usuario.
    InsecureDirectory,
    /// Ya existe un archivo de autorización; no se sobrescribe.
    AlreadyExists,
    /// El directorio no contiene una base candidata.
    MissingDatabase,
    /// No hay autorización entregada en el directorio.
    MissingAuthorization,
    /// La autorización no existe en la base.
    UnknownAuthorization,
    /// Entropía o criptografía no disponibles.
    Crypto,
    /// Vigencia fuera de política.
    InvalidWindow,
    /// Fallo de persistencia.
    Storage,
    /// Archivo administrativo ilegible o mal formado.
    InvalidFile,
}

impl fmt::Display for ProvisioningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Usage => "uso: --provision-token-issue|status|revoke <directorio>",
            Self::UnsupportedPlatform => {
                "verificación de custodia no disponible en esta plataforma; operación rechazada"
            }
            Self::InsecureDirectory => "el directorio no es privado o no pertenece al usuario",
            Self::AlreadyExists => "ya existe una autorización entregada en el directorio",
            Self::MissingDatabase => "el directorio no contiene una base candidata",
            Self::MissingAuthorization => "no hay autorización entregada en el directorio",
            Self::UnknownAuthorization => "la autorización no existe",
            Self::Crypto => "material criptográfico no disponible",
            Self::InvalidWindow => "vigencia fuera de política",
            Self::Storage => "persistencia de provisioning fallida",
            Self::InvalidFile => "archivo administrativo inválido",
        })
    }
}

impl std::error::Error for ProvisioningError {}

impl From<pos_credentials::CredentialError> for ProvisioningError {
    fn from(_: pos_credentials::CredentialError) -> Self {
        Self::Crypto
    }
}

impl From<ProvisioningError> for pos_core::identity::ProvisioningError {
    fn from(_: ProvisioningError) -> Self {
        pos_core::identity::ProvisioningError::IncoherentState
    }
}

/// Interpreta los argumentos del proceso; `None` significa ciclo de vida normal.
///
/// # Errors
/// Devuelve [`ProvisioningError::Usage`] si una opción administrativa está
/// incompleta o aparece más de una.
pub fn parse(args: &[String]) -> Result<Option<ProvisioningCommand>, ProvisioningError> {
    let mut command: Option<ProvisioningCommand> = None;
    let mut index = 1;
    while index < args.len() {
        let argument = args[index].as_str();
        let kind = match argument {
            "--provision-token-issue" => "issue",
            "--provision-token-status" => "status",
            "--provision-token-revoke" => "revoke",
            _ => {
                index += 1;
                continue;
            }
        };
        if command.is_some() {
            return Err(ProvisioningError::Usage);
        }
        let directory = args.get(index + 1).ok_or(ProvisioningError::Usage)?;
        if directory.starts_with("--") {
            return Err(ProvisioningError::Usage);
        }
        let directory = PathBuf::from(directory);
        command = Some(match kind {
            "issue" => ProvisioningCommand::Issue {
                directory,
                ttl_seconds: DEFAULT_TTL_SECONDS,
            },
            "status" => ProvisioningCommand::Status { directory },
            _ => ProvisioningCommand::Revoke { directory },
        });
        index += 2;
    }
    Ok(command)
}

/// Ejecuta la operación administrativa e imprime solo datos no sensibles.
///
/// # Errors
/// Propaga el fallo correspondiente sin incluir token, digest ni rutas secretas.
pub async fn execute(command: ProvisioningCommand) -> Result<(), ProvisioningError> {
    match command {
        ProvisioningCommand::Issue {
            directory,
            ttl_seconds,
        } => issue(&directory, ttl_seconds).await,
        ProvisioningCommand::Status { directory } => status(&directory).await,
        ProvisioningCommand::Revoke { directory } => revoke(&directory).await,
    }
}

async fn issue(directory: &std::path::Path, ttl_seconds: i64) -> Result<(), ProvisioningError> {
    prepare_directory(directory)?;
    let database = directory.join(DATABASE_FILE);
    if !database.is_file() {
        return Err(ProvisioningError::MissingDatabase);
    }
    let pool = open(&database, Duration::from_millis(75))
        .await
        .map_err(|_| ProvisioningError::Storage)?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let now = now_epoch()?;

    let installation_id = parse_id(&random_uuid_v4()?)?;
    let authorization_id = parse_id(&random_uuid_v4()?)?;
    let event_id = parse_id(&random_uuid_v4()?)?;
    let token = ProvisioningToken::generate()?;
    let expires_at = ProvisioningAuthorization::expires_from(now, ttl_seconds)
        .map_err(|_| ProvisioningError::InvalidWindow)?;

    store
        .ensure_provisional_installation(&installation_id, &format!("local:{installation_id}"), now)
        .await
        .map_err(|_| ProvisioningError::Storage)?;

    let authorization = ProvisioningAuthorization::issue(
        authorization_id.clone(),
        installation_id.clone(),
        ProvisioningPurpose::Bootstrap,
        &token.digest().hex(),
        now,
        expires_at,
    )
    .map_err(|_| ProvisioningError::InvalidWindow)?;
    store
        .issue(&authorization, &event_id, now)
        .await
        .map_err(|_| ProvisioningError::Storage)?;

    write_authorization_file(
        directory,
        installation_id.as_str(),
        authorization_id.as_str(),
        token.expose(),
    )?;
    // Solo datos no sensibles: identificadores y ruta del archivo.
    println!(
        "{{\"event\":\"provisioning_issued\",\"authorization_id\":\"{}\",\"installation_id\":\"{}\",\"expires_at\":{},\"file\":\"{}\"}}",
        authorization_id.as_str(),
        installation_id.as_str(),
        expires_at,
        AUTHORIZATION_FILE
    );
    pool.close().await;
    Ok(())
}

async fn status(directory: &std::path::Path) -> Result<(), ProvisioningError> {
    prepare_directory(directory)?;
    let authorization_id = read_authorization_id(directory)?;
    let pool = open(&directory.join(DATABASE_FILE), Duration::from_millis(75))
        .await
        .map_err(|_| ProvisioningError::Storage)?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let authorization = store
        .get(&authorization_id)
        .await
        .map_err(|_| ProvisioningError::Storage)?
        .ok_or(ProvisioningError::UnknownAuthorization)?;
    println!(
        "{{\"event\":\"provisioning_status\",\"authorization_id\":\"{}\",\"state\":\"{}\",\"issued_at\":{},\"expires_at\":{}}}",
        authorization.id().as_str(),
        authorization.state().as_str(),
        authorization.issued_at(),
        authorization.expires_at()
    );
    pool.close().await;
    Ok(())
}

async fn revoke(directory: &std::path::Path) -> Result<(), ProvisioningError> {
    prepare_directory(directory)?;
    let authorization_id = read_authorization_id(directory)?;
    let pool = open(&directory.join(DATABASE_FILE), Duration::from_millis(75))
        .await
        .map_err(|_| ProvisioningError::Storage)?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let event_id = parse_id(&random_uuid_v4()?)?;
    let state = store
        .revoke(&authorization_id, &event_id, now_epoch()?)
        .await
        .map_err(|_| ProvisioningError::Storage)?;
    println!(
        "{{\"event\":\"provisioning_revoked\",\"authorization_id\":\"{}\",\"state\":\"{}\"}}",
        authorization_id.as_str(),
        state.as_str()
    );
    pool.close().await;
    Ok(())
}

fn parse_id(value: &str) -> Result<EntityId, ProvisioningError> {
    EntityId::parse(value).map_err(|_| ProvisioningError::Crypto)
}

fn now_epoch() -> Result<i64, ProvisioningError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProvisioningError::InvalidWindow)?;
    i64::try_from(elapsed.as_secs()).map_err(|_| ProvisioningError::InvalidWindow)
}

#[cfg(unix)]
fn prepare_directory(path: &std::path::Path) -> Result<(), ProvisioningError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(ProvisioningError::InsecureDirectory);
    }
    fs::create_dir_all(path).map_err(|_| ProvisioningError::InsecureDirectory)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| ProvisioningError::InsecureDirectory)?;
    if !metadata.is_dir() {
        return Err(ProvisioningError::InsecureDirectory);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(ProvisioningError::InsecureDirectory);
    }
    if metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Err(ProvisioningError::InsecureDirectory);
    }
    Ok(())
}

#[cfg(not(unix))]
fn prepare_directory(_path: &std::path::Path) -> Result<(), ProvisioningError> {
    Err(ProvisioningError::UnsupportedPlatform)
}

#[cfg(unix)]
fn write_authorization_file(
    directory: &std::path::Path,
    installation_id: &str,
    authorization_id: &str,
    token: &str,
) -> Result<(), ProvisioningError> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let path = directory.join(AUTHORIZATION_FILE);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ProvisioningError::AlreadyExists
            } else {
                ProvisioningError::InsecureDirectory
            }
        })?;
    writeln!(file, "installation_id={installation_id}")
        .and_then(|()| writeln!(file, "authorization_id={authorization_id}"))
        .and_then(|()| writeln!(file, "token={token}"))
        .map_err(|_| ProvisioningError::Storage)?;
    file.sync_all().map_err(|_| ProvisioningError::Storage)
}

#[cfg(not(unix))]
fn write_authorization_file(
    _directory: &std::path::Path,
    _installation_id: &str,
    _authorization_id: &str,
    _token: &str,
) -> Result<(), ProvisioningError> {
    Err(ProvisioningError::UnsupportedPlatform)
}

fn read_authorization_id(directory: &std::path::Path) -> Result<EntityId, ProvisioningError> {
    let contents = fs::read_to_string(directory.join(AUTHORIZATION_FILE))
        .map_err(|_| ProvisioningError::MissingAuthorization)?;
    let value = contents
        .lines()
        .find_map(|line| line.strip_prefix("authorization_id="))
        .ok_or(ProvisioningError::InvalidFile)?;
    parse_id(value.trim())
}
