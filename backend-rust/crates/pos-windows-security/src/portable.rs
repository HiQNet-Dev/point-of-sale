//! Implementación para plataformas sin custodia nativa verificable.
//!
//! El producto aplica su propia política POSIX en Unix dentro de cada
//! entrypoint, por lo que este crate no interviene. Estas funciones existen para
//! que los llamadores que solo se compilan en Windows conserven una firma
//! estable y para que cualquier otra plataforma falle cerrada en lugar de
//! aceptar silenciosamente una ruta sin verificar.

use std::path::Path;

use crate::SecurityError;

/// La plataforma no expone la custodia de archivos privados en este crate.
///
/// # Errors
/// Siempre devuelve [`SecurityError::UnsupportedPlatform`].
pub fn effective_user_sid() -> Result<String, SecurityError> {
    Err(SecurityError::UnsupportedPlatform)
}

/// La plataforma no expone la custodia de archivos privados en este crate.
///
/// # Errors
/// Siempre devuelve [`SecurityError::UnsupportedPlatform`].
pub fn create_private_directory(_path: &Path) -> Result<(), SecurityError> {
    Err(SecurityError::UnsupportedPlatform)
}

/// La plataforma no expone la custodia de archivos privados en este crate.
///
/// # Errors
/// Siempre devuelve [`SecurityError::UnsupportedPlatform`].
pub fn verify_private_directory(_path: &Path) -> Result<(), SecurityError> {
    Err(SecurityError::UnsupportedPlatform)
}

/// La plataforma no expone la custodia de archivos privados en este crate.
///
/// # Errors
/// Siempre devuelve [`SecurityError::UnsupportedPlatform`].
pub fn create_private_file(
    _directory: &Path,
    _name: &str,
    _contents: &[u8],
) -> Result<(), SecurityError> {
    Err(SecurityError::UnsupportedPlatform)
}

/// La plataforma no expone la custodia de archivos privados en este crate.
///
/// # Errors
/// Siempre devuelve [`SecurityError::UnsupportedPlatform`].
pub fn read_private_file(_directory: &Path, _name: &str) -> Result<Vec<u8>, SecurityError> {
    Err(SecurityError::UnsupportedPlatform)
}

/// La plataforma no expone la custodia de archivos privados en este crate.
///
/// # Errors
/// Siempre devuelve [`SecurityError::UnsupportedPlatform`].
pub fn verify_private_file(_directory: &Path, _name: &str) -> Result<(), SecurityError> {
    Err(SecurityError::UnsupportedPlatform)
}
