//! Frontera de plataforma para la custodia de archivos privados de HiQNet POS.
//!
//! # Alcance
//!
//! Este crate expone la única API de seguridad de sistema de archivos usada por
//! el producto Free Local en Windows: creación privada de directorios y
//! archivos, verificación de propietario efectivo y DACL, rechazo de enlaces y
//! puntos de reanálisis, y apertura protegida frente a sustituciones de ruta.
//!
//! En Unix el producto ya aplica su propia política POSIX en cada entrypoint
//! (bits de permiso y propietario del usuario efectivo) y este crate no
//! participa; en el resto de plataformas toda operación falla cerrada con
//! [`SecurityError::UnsupportedPlatform`].
//!
//! # Política de acceso admitida
//!
//! Un objeto es privado cuando:
//!
//! - su propietario es el usuario efectivo del proceso;
//! - su DACL no contiene ninguna entrada de *concesión* a una identidad ajena.
//!
//! Las únicas identidades admitidas son el propio usuario, `SYSTEM`
//! (`S-1-5-18`) y el grupo `Administrators` (`S-1-5-32-544`). Cualquier otra
//! concesión —`Everyone`, `Users`, `Authenticated Users`, `INTERACTIVE`,
//! servicios, grupos personalizados— se rechaza, con independencia de los
//! permisos concretos concedidos. Las entradas de *denegación* y las etiquetas
//! de integridad se ignoran porque no amplían el acceso.
//!
//! # Límites explícitos
//!
//! La política no protege frente a un administrador del equipo ni frente a
//! código que se ejecute con la propia identidad del usuario: ambos pueden
//! tomar posesión del objeto. Tampoco sustituye al cifrado en reposo, que no
//! forma parte de este contrato.

use std::path::Path;

/// Fallo de custodia; nunca contiene la ruta ni material sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityError {
    /// La plataforma no expone la verificación de propietario y DACL.
    UnsupportedPlatform,
    /// El objeto no existe.
    Missing,
    /// La ruta existe pero no es del tipo esperado.
    NotExpectedType,
    /// La ruta atraviesa o es un enlace simbólico, junction o punto de reanálisis.
    Redirect,
    /// El propietario no es el usuario efectivo del proceso.
    WrongOwner,
    /// La DACL concede acceso a una identidad no autorizada.
    InsecureDacl,
    /// El nombre solicitado no es un componente simple de ruta.
    InvalidName,
    /// El objeto ya existía y la operación exigía crearlo.
    AlreadyExists,
    /// La llamada nativa falló con un código que no clasificamos con más detalle.
    Native,
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "custodia de archivos no disponible en esta plataforma",
            Self::Missing => "el archivo de instalación no existe",
            Self::NotExpectedType => "la ruta no es del tipo esperado",
            Self::Redirect => "la ruta es un enlace o punto de reanálisis",
            Self::WrongOwner => "el propietario del archivo no es el usuario de la instalación",
            Self::InsecureDacl => "el archivo concede acceso a otras identidades",
            Self::InvalidName => "nombre de archivo inválido",
            Self::AlreadyExists => "el archivo ya existe",
            Self::Native => "la plataforma rechazó la operación de seguridad",
        })
    }
}

impl std::error::Error for SecurityError {}

/// Interpretación en Rust seguro de los descriptores SDDL.
#[cfg(any(windows, test))]
mod sddl;

#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::{
    create_private_directory, create_private_file, effective_user_sid, read_private_file,
    verify_private_directory, verify_private_file,
};

#[cfg(not(windows))]
mod portable;

#[cfg(not(windows))]
pub use portable::{
    create_private_directory, create_private_file, effective_user_sid, read_private_file,
    verify_private_directory, verify_private_file,
};

/// Comprueba que `name` es un único componente de ruta sin separadores.
///
/// Evita que un llamador escape del directorio validado mediante `..`, rutas
/// absolutas o separadores alternativos.
///
/// # Errors
/// Devuelve [`SecurityError::InvalidName`] si el nombre está vacío, contiene
/// separadores, es `..` o contiene bytes nulos.
pub fn validate_file_name(name: &str) -> Result<(), SecurityError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('\0') {
        return Err(SecurityError::InvalidName);
    }
    if name.contains('/') || name.contains('\\') || name.contains(':') {
        return Err(SecurityError::InvalidName);
    }
    if Path::new(name).components().count() != 1 {
        return Err(SecurityError::InvalidName);
    }
    Ok(())
}

#[cfg(test)]
mod name_tests {
    use super::{SecurityError, validate_file_name};

    #[test]
    fn accepts_plain_file_names() {
        assert_eq!(validate_file_name("provisioning-authorization.txt"), Ok(()));
        assert_eq!(validate_file_name("hiqnet-pos.sqlite-wal"), Ok(()));
        assert_eq!(validate_file_name(".hiqnet-pos.lock"), Ok(()));
    }

    #[test]
    fn rejects_traversal_and_alternate_separators() {
        for name in [
            "",
            ".",
            "..",
            "../secreto",
            "sub/archivo",
            "sub\\archivo",
            "C:archivo",
            "C:\\Windows\\archivo",
            "/etc/passwd",
            "archivo\0.txt",
        ] {
            assert_eq!(
                validate_file_name(name),
                Err(SecurityError::InvalidName),
                "se esperaba rechazo para {name:?}"
            );
        }
    }
}
