//! Lectura protegida del archivo de autorización de provisioning.
//!
//! El emisor administrativo (`pos-local --provision-token-issue`) entrega el
//! token en un archivo privado dentro del directorio de la instalación. Este
//! módulo lo lee desde el proceso nativo —nunca desde JavaScript— y aplica la
//! misma política de custodia en Unix:
//!
//! - el directorio no puede ser un enlace;
//! - el directorio no puede conceder bits de grupo/otros y debe pertenecer al
//!   usuario efectivo;
//! - el archivo debe ser regular, no un enlace, privado y del mismo owner.
//!
//! En Windows el comportamiento es **fail-closed** hasta que exista el
//! componente nativo de DACL/owner aprobado.

use std::path::Path;

use pos_core::{
    identity::{ProvisioningPurpose, ProvisioningState},
    organization_branch::EntityId,
};
use pos_credentials::ProvisioningToken;
use sqlx::SqlitePool;

/// Fallo de custodia o lectura del archivo de autorización.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationFileError {
    /// La plataforma no permite verificar owner/DACL todavía.
    UnsupportedPlatform,
    /// Directorio o archivo con permisos, owner o tipo inseguros.
    Insecure,
    /// No existe el archivo de autorización.
    Missing,
    /// El archivo está mal formado o el token no es canónico.
    Invalid,
}

impl std::fmt::Display for AuthorizationFileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "custodia de archivos no disponible en esta plataforma",
            Self::Insecure => "el archivo de autorización no es privado",
            Self::Missing => "no hay autorización de provisioning entregada",
            Self::Invalid => "la autorización de provisioning es ilegible",
        })
    }
}

impl std::error::Error for AuthorizationFileError {}

/// Autorización nativa lista para el bootstrap.
///
/// El token se conserva en memoria del proceso y se borra al destruirse; nunca
/// se serializa, registra ni envía al renderer.
pub struct NativeAuthorization {
    /// Instalación a la que pertenece el derecho.
    pub installation_id: EntityId,
    /// Identificador de la autorización persistida.
    pub authorization_id: EntityId,
    /// Token presentado al consumir el derecho.
    pub token: ProvisioningToken,
}

impl std::fmt::Debug for NativeAuthorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeAuthorization")
            .field("installation_id", &self.installation_id)
            .field("authorization_id", &self.authorization_id)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// Nombre del archivo de autorización entregado por el emisor nativo.
pub const NATIVE_AUTHORIZATION_FILE: &str = "provisioning-authorization.txt";

/// Estado agregado de una autorización nativa frente a la base.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeAuthorizationState {
    /// Derecho vivo y vigente: puede consumirse.
    Ready {
        /// Fin de la ventana, en segundos Unix.
        expires_at: i64,
    },
    /// Derecho vivo pero caducado.
    Expired,
    /// Derecho revocado por el operador.
    Revoked,
    /// Derecho ya consumido por un bootstrap anterior.
    Consumed,
    /// El archivo no coincide con ningún derecho persistido.
    Unknown,
}

/// Lee la autorización nativa del directorio de instalación.
///
/// # Errors
/// Devuelve [`AuthorizationFileError`] si la plataforma no permite verificar la
/// custodia, el archivo no es privado, falta o está mal formado.
pub fn read_native_authorization(
    directory: &Path,
) -> Result<NativeAuthorization, AuthorizationFileError> {
    validate_directory(directory)?;
    let path = directory.join(NATIVE_AUTHORIZATION_FILE);
    validate_file(&path)?;
    let contents = std::fs::read_to_string(&path).map_err(|_| AuthorizationFileError::Missing)?;
    parse_authorization(&contents)
}

/// Consulta el estado del derecho sin consumirlo.
///
/// # Errors
/// Devuelve [`ProvisioningStorageError::Database`] si SQLite no responde.
pub async fn native_authorization_state(
    pool: &SqlitePool,
    authorization: &NativeAuthorization,
    now: i64,
) -> Result<NativeAuthorizationState, crate::provisioning::ProvisioningStorageError> {
    use crate::provisioning::SqliteProvisioningStore;
    let store = SqliteProvisioningStore::new(pool.clone());
    let Some(stored) = store.get(&authorization.authorization_id).await? else {
        return Ok(NativeAuthorizationState::Unknown);
    };
    if stored.installation_id() != &authorization.installation_id
        || stored.purpose() != ProvisioningPurpose::Bootstrap
        || !stored
            .token_digest()
            .eq_ignore_ascii_case(&authorization.token.digest().hex())
    {
        return Ok(NativeAuthorizationState::Unknown);
    }
    Ok(match stored.state() {
        ProvisioningState::Pending => {
            if stored.expires_at() <= now {
                NativeAuthorizationState::Expired
            } else {
                NativeAuthorizationState::Ready {
                    expires_at: stored.expires_at(),
                }
            }
        }
        ProvisioningState::Expired => NativeAuthorizationState::Expired,
        ProvisioningState::Revoked => NativeAuthorizationState::Revoked,
        ProvisioningState::Consumed => NativeAuthorizationState::Consumed,
    })
}

fn parse_authorization(contents: &str) -> Result<NativeAuthorization, AuthorizationFileError> {
    let mut installation_id = None;
    let mut authorization_id = None;
    let mut token = None;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("installation_id=") {
            installation_id = Some(value.trim());
        } else if let Some(value) = line.strip_prefix("authorization_id=") {
            authorization_id = Some(value.trim());
        } else if let Some(value) = line.strip_prefix("token=") {
            token = Some(value.trim());
        }
    }
    let installation_id = EntityId::parse(installation_id.ok_or(AuthorizationFileError::Invalid)?)
        .map_err(|_| AuthorizationFileError::Invalid)?;
    let authorization_id =
        EntityId::parse(authorization_id.ok_or(AuthorizationFileError::Invalid)?)
            .map_err(|_| AuthorizationFileError::Invalid)?;
    let token = ProvisioningToken::from_encoded(token.ok_or(AuthorizationFileError::Invalid)?)
        .map_err(|_| AuthorizationFileError::Invalid)?;
    Ok(NativeAuthorization {
        installation_id,
        authorization_id,
        token,
    })
}

#[cfg(unix)]
fn validate_directory(path: &Path) -> Result<(), AuthorizationFileError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = path
        .symlink_metadata()
        .map_err(|_| AuthorizationFileError::Missing)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AuthorizationFileError::Insecure);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(AuthorizationFileError::Insecure);
    }
    if metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Err(AuthorizationFileError::Insecure);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_directory(_path: &Path) -> Result<(), AuthorizationFileError> {
    Err(AuthorizationFileError::UnsupportedPlatform)
}

#[cfg(unix)]
fn validate_file(path: &Path) -> Result<(), AuthorizationFileError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AuthorizationFileError::Missing);
        }
        Err(_) => return Err(AuthorizationFileError::Invalid),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AuthorizationFileError::Insecure);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(AuthorizationFileError::Insecure);
    }
    if metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Err(AuthorizationFileError::Insecure);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_file(_path: &Path) -> Result<(), AuthorizationFileError> {
    Err(AuthorizationFileError::UnsupportedPlatform)
}
