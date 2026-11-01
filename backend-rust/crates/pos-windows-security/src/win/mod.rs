//! Custodia de archivos privados en Windows.
//!
//! La política se expresa aquí sin `unsafe`: se abre el objeto, se comprueba el
//! tipo real y el punto de reanálisis sobre el handle, se compara el propietario
//! con el usuario efectivo del proceso y se recorren las concesiones de la DACL.
//! Las llamadas nativas viven en [`native`].
//!
//! # Invariante de sustitución
//!
//! Toda verificación se hace sobre el handle que se va a usar, no sobre la ruta:
//!
//! 1. se abre el directorio con `FILE_FLAG_OPEN_REPARSE_POINT`, de modo que un
//!    enlace o junction se detecta en lugar de seguirse;
//! 2. se abre el archivo del mismo modo y se valida su handle;
//! 3. se compara la ruta final del archivo con la del directorio validado.
//!
//! Un atacante puede sustituir la ruta, pero necesitaría un objeto propiedad de
//! la misma cuenta y con una DACL privada, algo que no puede fabricar. La
//! ventana entre comprobar y usar queda por tanto cerrada para el contenido que
//! se lee o escribe.

use std::path::Path;

use crate::{SecurityError, validate_file_name};

use self::native::{Handle, OwnedHandle};

/// Implementación nativa: FFI, handles y descriptores.
mod native;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use super::sddl::{AceEffect, is_acceptable_owner, is_allowed_identity, parse_dacl};

/// SID del usuario efectivo del proceso en forma `S-1-...`.
///
/// # Errors
/// Devuelve [`SecurityError::Native`] si el token del proceso no puede
/// consultarse.
pub fn effective_user_sid() -> Result<String, SecurityError> {
    native::current_user_sid()
}

/// Crea el directorio si falta y exige que quede privado.
///
/// Los directorios intermedios se crean con la herencia habitual del perfil; el
/// directorio final recibe una DACL protegida con el usuario, `SYSTEM` y
/// `Administrators`, aplicada de forma atómica al crearse.
///
/// # Errors
/// Devuelve [`SecurityError`] si la creación o la verificación posterior fallan.
/// Un directorio preexistente nunca se corrige en silencio: si no es privado, se
/// rechaza.
pub fn create_private_directory(path: &Path) -> Result<(), SecurityError> {
    let owner = native::current_user_sid()?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|_| SecurityError::Native)?;
    }
    match native::create_directory_with(path, &private_sddl(&owner)) {
        Ok(()) | Err(SecurityError::AlreadyExists) => {}
        Err(error) => return Err(error),
    }
    verify_private_directory(path)
}

/// Verifica que un directorio existe, no es una redirección y es privado.
///
/// # Errors
/// Devuelve [`SecurityError::Missing`] si no existe,
/// [`SecurityError::Redirect`] si es un enlace o punto de reanálisis,
/// [`SecurityError::NotExpectedType`] si es un archivo y
/// [`SecurityError::WrongOwner`] o [`SecurityError::InsecureDacl`] si la custodia
/// no es privada.
pub fn verify_private_directory(path: &Path) -> Result<(), SecurityError> {
    let handle = native::open_directory(path)?;
    assert_directory(handle.raw())?;
    let owner = native::current_user_sid()?;
    assert_private(handle.raw(), &owner)
}

/// Verifica un archivo existente del directorio privado indicado.
///
/// # Errors
/// Devuelve [`SecurityError`] con la misma clasificación que
/// [`read_private_file`], más [`SecurityError::InvalidName`] si `name` no es un
/// componente simple.
pub fn verify_private_file(directory: &Path, name: &str) -> Result<(), SecurityError> {
    open_verified_file(directory, name).map(|_| ())
}

/// Lee un archivo privado validando el handle del que se leen los bytes.
///
/// # Errors
/// Devuelve [`SecurityError`] si el nombre, el directorio, el archivo o su
/// custodia no son válidos.
pub fn read_private_file(directory: &Path, name: &str) -> Result<Vec<u8>, SecurityError> {
    let file = open_verified_file(directory, name)?;
    native::read_all(file.raw())
}

/// Crea un archivo nuevo, exclusivo y privado dentro de un directorio privado.
///
/// El archivo nace con la DACL final en la misma operación de creación: no hay
/// ningún instante en el que el material sensible exista con una DACL heredada.
///
/// # Errors
/// Devuelve [`SecurityError::AlreadyExists`] si el archivo ya existe y
/// [`SecurityError`] si el directorio o la escritura fallan.
pub fn create_private_file(
    directory: &Path,
    name: &str,
    contents: &[u8],
) -> Result<(), SecurityError> {
    validate_file_name(name)?;
    let directory_handle = native::open_directory(directory)?;
    assert_directory(directory_handle.raw())?;
    let owner = native::current_user_sid()?;
    // Escribir un secreto en un directorio compartido lo expondría aunque el
    // archivo naciera privado: el directorio debe serlo antes de crearlo.
    assert_private(directory_handle.raw(), &owner)?;

    let file = native::create_file_with(&directory.join(name), &private_sddl(&owner))?;
    assert_regular_file(file.raw())?;
    assert_private(file.raw(), &owner)?;
    assert_inside(directory_handle.raw(), file.raw(), name)?;
    native::write_all(file.raw(), contents)
}

fn open_verified_file(directory: &Path, name: &str) -> Result<OwnedHandle, SecurityError> {
    validate_file_name(name)?;
    let directory_handle = native::open_directory(directory)?;
    assert_directory(directory_handle.raw())?;
    let file = native::open_regular_file(&directory.join(name))?;
    assert_regular_file(file.raw())?;
    let owner = native::current_user_sid()?;
    assert_private(directory_handle.raw(), &owner)?;
    assert_private(file.raw(), &owner)?;
    assert_inside(directory_handle.raw(), file.raw(), name)?;
    Ok(file)
}

/// DACL privada: propietario explícito, `SYSTEM` y `Administrators`, sin
/// herencia del padre pero heredable por los hijos.
///
/// El propietario se declara explícitamente para que el objeto quede a nombre
/// del usuario efectivo aunque el proceso se ejecute elevado; un proceso
/// elevado usaría `Administrators` como propietario por defecto.
///
/// `P` protege el objeto de heredar entradas del padre; `OICI` propaga la misma
/// DACL a los archivos y subdirectorios creados dentro (por ejemplo, el propio
/// archivo SQLite y sus auxiliares); `FA` concede control total a esas tres
/// identidades, que son las únicas admitidas por la política.
fn private_sddl(owner_sid: &str) -> String {
    format!("O:{owner_sid}D:P(A;OICI;FA;;;{owner_sid})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)")
}

fn assert_directory(handle: Handle) -> Result<(), SecurityError> {
    let attributes = native::attributes(handle)?;
    if attributes.is_reparse_point() {
        return Err(SecurityError::Redirect);
    }
    if !attributes.is_directory() {
        return Err(SecurityError::NotExpectedType);
    }
    Ok(())
}

fn assert_regular_file(handle: Handle) -> Result<(), SecurityError> {
    let attributes = native::attributes(handle)?;
    if attributes.is_reparse_point() {
        return Err(SecurityError::Redirect);
    }
    if attributes.is_directory() {
        return Err(SecurityError::NotExpectedType);
    }
    Ok(())
}

fn assert_private(handle: Handle, owner_sid: &str) -> Result<(), SecurityError> {
    let (actual_owner, sddl) = native::owner_and_sddl(handle)?;
    if !is_acceptable_owner(&actual_owner, owner_sid) {
        return Err(SecurityError::WrongOwner);
    }
    let aces = parse_dacl(&sddl).map_err(|()| SecurityError::InsecureDacl)?;
    for ace in aces {
        if ace.effect != AceEffect::Grant {
            continue;
        }
        // El descriptor puede nombrar identidades por alias (`LA`, `BA`…). Un
        // alias que no se pueda resolver se trata como concesión no autorizada:
        // nunca se asume que un descriptor ilegible es privado.
        let sid = native::sid_from_text(&ace.account).map_err(|_| SecurityError::InsecureDacl)?;
        if !is_allowed_identity(&sid, owner_sid) {
            return Err(SecurityError::InsecureDacl);
        }
    }
    Ok(())
}

/// Comprueba que el archivo abierto está realmente dentro del directorio
/// validado, comparando las rutas finales que resuelve el sistema.
fn assert_inside(directory: Handle, file: Handle, name: &str) -> Result<(), SecurityError> {
    let parent = native::final_path(directory)?;
    let expected = format!("{}\\{name}", parent.trim_end_matches('\\'));
    let actual = native::final_path(file)?;
    match actual.get(..expected.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(&expected) => Ok(()),
        _ => Err(SecurityError::Redirect),
    }
}
