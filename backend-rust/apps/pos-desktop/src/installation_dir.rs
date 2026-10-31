//! Validación del directorio de instalación Local.
//!
//! El modo producto no acepta rutas arbitrarias desde JavaScript ni desde el
//! entorno: el directorio llega por argumento explícito y se valida antes de
//! abrir SQLite.
//!
//! # Reglas
//!
//! - Debe ser un directorio real, nunca un archivo ni un enlace simbólico.
//! - En Windows también se rechazan *junctions* y otros puntos de reanálisis.
//! - Se crea si falta, con permisos de propietario en Unix.
//! - Se rechaza un directorio escribible por todos: otro usuario local podría
//!   sustituir el archivo de la instalación.
//! - El archivo de base de datos tampoco puede ser un enlace simbólico.
//!
//! # Límites conocidos
//!
//! El bloqueo entre procesos lo aporta SQLite (un pool por proceso y
//! `BEGIN IMMEDIATE` para escrituras críticas); este módulo no implementa un
//! candado de instancia única.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

/// Nombre fijo del archivo dentro del directorio de instalación.
pub const DATABASE_FILE_NAME: &str = "hiqnet-pos.sqlite";

/// Directorio estable de producto bajo el directorio de datos del sistema.
///
/// Es el candidato `com.hiqnet.pos` del bloque D. Se usa **solo** como nombre de
/// directorio: el bundle identifier de Tauri sigue siendo el de laboratorio
/// hasta verificar colisiones, continuidad de almacenamiento y requisitos de
/// distribución. Reutilizar el identificador de laboratorio ubicaría datos de
/// producto en una ruta experimental.
pub const PRODUCT_DATA_DIRECTORY: &str = "com.hiqnet.pos";

/// Directorio validado de una instalación Local.
#[derive(Clone, Debug)]
pub struct InstallationDirectory {
    path: PathBuf,
}

/// Fallos de validación sin contenido de la ruta en su representación.
#[derive(Debug)]
pub enum DirectoryError {
    /// La ruta está vacía, es un archivo o no puede usarse como directorio.
    InvalidPath,
    /// La ruta o el archivo de base de datos es un enlace simbólico.
    Symlink,
    /// Los permisos permiten que otro usuario sustituya la instalación.
    #[cfg_attr(not(unix), allow(dead_code))] // Solo evaluable con bits POSIX.
    InsecurePermissions,
    /// El sistema operativo no expone un directorio de datos para el usuario.
    ApplicationDataUnavailable,
    /// El sistema de archivos rechazó la operación.
    Unavailable,
}

impl std::fmt::Display for DirectoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPath => "directorio de instalación inválido",
            Self::Symlink => "el directorio de instalación no puede ser un enlace simbólico",
            Self::InsecurePermissions => {
                "el directorio de instalación es accesible por otros usuarios"
            }
            Self::ApplicationDataUnavailable => {
                "el sistema no ofrece un directorio de datos de aplicación"
            }
            Self::Unavailable => "el directorio de instalación no está disponible",
        })
    }
}

impl std::error::Error for DirectoryError {}

/// Directorio privado que el sistema operativo asigna al usuario actual,
/// seguido del directorio estable del producto.
///
/// Solo resuelve la ruta candidata: no la crea ni la valida. La validación de
/// enlaces, permisos y archivo corresponde a [`InstallationDirectory::resolve`].
///
/// # Errors
/// Devuelve [`DirectoryError::ApplicationDataUnavailable`] si el sistema no
/// expone un directorio de datos para el usuario.
pub fn default_data_directory() -> Result<PathBuf, DirectoryError> {
    Ok(platform_data_root(|name| std::env::var_os(name))?.join(PRODUCT_DATA_DIRECTORY))
}

/// Raíz de datos por plataforma, con el lector de entorno inyectado.
///
/// La inyección permite probar la política en cualquier sistema sin mutar el
/// entorno del proceso.
fn platform_data_root<F>(lookup: F) -> Result<PathBuf, DirectoryError>
where
    F: Fn(&str) -> Option<OsString>,
{
    #[cfg(windows)]
    {
        if let Some(root) = usable(lookup("LOCALAPPDATA")) {
            return Ok(PathBuf::from(root));
        }
        // Un perfil sin LOCALAPPDATA conserva la misma ubicación privada.
        let profile =
            usable(lookup("USERPROFILE")).ok_or(DirectoryError::ApplicationDataUnavailable)?;
        Ok(PathBuf::from(profile).join("AppData").join("Local"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = usable(lookup("HOME")).ok_or(DirectoryError::ApplicationDataUnavailable)?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(root) = usable(lookup("XDG_DATA_HOME")) {
            return Ok(PathBuf::from(root));
        }
        let home = usable(lookup("HOME")).ok_or(DirectoryError::ApplicationDataUnavailable)?;
        Ok(PathBuf::from(home).join(".local").join("share"))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = lookup;
        Err(DirectoryError::ApplicationDataUnavailable)
    }
}

fn usable(value: Option<OsString>) -> Option<OsString> {
    value.filter(|candidate| !candidate.is_empty())
}

impl InstallationDirectory {
    /// Valida (y crea si falta) el directorio indicado.
    ///
    /// # Errors
    /// Devuelve [`DirectoryError`] si la ruta no es un directorio seguro.
    pub fn resolve(path: &Path) -> Result<Self, DirectoryError> {
        if path.as_os_str().is_empty() {
            return Err(DirectoryError::InvalidPath);
        }
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if is_redirect(&metadata) {
                    return Err(DirectoryError::Symlink);
                }
                if !metadata.is_dir() {
                    return Err(DirectoryError::InvalidPath);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(path).map_err(|_| DirectoryError::Unavailable)?;
                restrict_to_owner(path)?;
            }
            Err(_) => return Err(DirectoryError::Unavailable),
        }
        ensure_not_world_writable(path)?;
        let directory = Self {
            path: path.to_path_buf(),
        };
        directory.validate_database_file()?;
        Ok(directory)
    }

    /// Ruta del archivo SQLite de esta instalación.
    pub fn database_file(&self) -> PathBuf {
        self.path.join(DATABASE_FILE_NAME)
    }

    /// El archivo no puede ser un enlace ni un directorio.
    fn validate_database_file(&self) -> Result<(), DirectoryError> {
        let database = self.database_file();
        match fs::symlink_metadata(&database) {
            Ok(metadata) => {
                if is_redirect(&metadata) {
                    return Err(DirectoryError::Symlink);
                }
                if metadata.is_dir() {
                    return Err(DirectoryError::InvalidPath);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(DirectoryError::Unavailable),
        }
        Ok(())
    }
}

/// Un enlace simbólico o, en Windows, cualquier punto de reanálisis (junction).
fn is_redirect(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: cubre junctions y mount points, que
        // `is_symlink()` no reporta.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> Result<(), DirectoryError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| DirectoryError::Unavailable)
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> Result<(), DirectoryError> {
    // En Windows el ACL heredado del perfil del usuario ya excluye a otros usuarios.
    Ok(())
}

#[cfg(unix)]
fn ensure_not_world_writable(path: &Path) -> Result<(), DirectoryError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)
        .map_err(|_| DirectoryError::Unavailable)?
        .permissions()
        .mode();
    if mode & 0o002 != 0 {
        return Err(DirectoryError::InsecurePermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_not_world_writable(_path: &Path) -> Result<(), DirectoryError> {
    // Sin bits POSIX no se puede evaluar el ACL sin dependencias adicionales.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_missing_directory_and_names_the_database() -> Result<(), Box<dyn std::error::Error>>
    {
        let parent = tempfile::tempdir()?;
        let target = parent.path().join("hiqnet");
        let directory = InstallationDirectory::resolve(&target)?;
        assert!(target.is_dir());
        assert_eq!(directory.database_file(), target.join(DATABASE_FILE_NAME));
        Ok(())
    }

    #[test]
    fn rejects_a_file_and_an_empty_path() -> Result<(), Box<dyn std::error::Error>> {
        let parent = tempfile::tempdir()?;
        let file = parent.path().join("archivo");
        std::fs::write(&file, b"x")?;
        assert!(matches!(
            InstallationDirectory::resolve(&file),
            Err(DirectoryError::InvalidPath)
        ));
        assert!(matches!(
            InstallationDirectory::resolve(Path::new("")),
            Err(DirectoryError::InvalidPath)
        ));
        Ok(())
    }

    #[test]
    fn rejects_a_symlinked_database_file() -> Result<(), Box<dyn std::error::Error>> {
        let parent = tempfile::tempdir()?;
        let directory = InstallationDirectory::resolve(&parent.path().join("instalacion"))?;
        let real = parent.path().join("ajeno.sqlite");
        std::fs::write(&real, b"x")?;
        if create_symlink(&real, &directory.database_file()).is_err() {
            // Sin privilegios de enlace en este entorno: la regla sigue valiendo.
            return Ok(());
        }
        assert!(matches!(
            InstallationDirectory::resolve(parent.path().join("instalacion").as_path()),
            Err(DirectoryError::Symlink)
        ));
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn rejects_a_windows_junction() -> Result<(), Box<dyn std::error::Error>> {
        let parent = tempfile::tempdir()?;
        let target = parent.path().join("real");
        std::fs::create_dir(&target)?;
        let junction = parent.path().join("enlace");
        // Los junctions no requieren privilegios de administrador; si el entorno
        // no los permite, la regla de atributos sigue aplicándose al código.
        let created = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output();
        if created.is_err() || !created.is_ok_and(|output| output.status.success()) {
            return Ok(());
        }
        assert!(matches!(
            InstallationDirectory::resolve(&junction),
            Err(DirectoryError::Symlink)
        ));
        Ok(())
    }

    #[cfg(unix)]
    fn create_symlink(target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_symlink(target: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_world_writable_directory() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let parent = tempfile::tempdir()?;
        let target = parent.path().join("compartida");
        std::fs::create_dir(&target)?;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o777))?;
        assert!(matches!(
            InstallationDirectory::resolve(&target),
            Err(DirectoryError::InsecurePermissions)
        ));
        Ok(())
    }

    #[test]
    fn product_directory_is_not_the_laboratory_identifier() {
        assert_eq!(PRODUCT_DATA_DIRECTORY, "com.hiqnet.pos");
        assert_ne!(PRODUCT_DATA_DIRECTORY, "local.hiqnet.rustlab");
        assert!(PRODUCT_DATA_DIRECTORY.contains('.'));
        assert!(!PRODUCT_DATA_DIRECTORY.contains('/'));
        assert!(!PRODUCT_DATA_DIRECTORY.contains('\\'));
    }

    #[cfg(windows)]
    #[test]
    fn windows_root_prefers_local_app_data_and_falls_back_to_the_profile() {
        let preferred = platform_data_root(|name| match name {
            "LOCALAPPDATA" => Some(OsString::from("C:/Users/demo/AppData/Local")),
            _ => None,
        });
        assert_eq!(
            preferred.ok().as_deref(),
            Some(Path::new("C:/Users/demo/AppData/Local"))
        );
        let fallback = platform_data_root(|name| match name {
            "USERPROFILE" => Some(OsString::from("C:/Users/demo")),
            _ => None,
        });
        assert_eq!(
            fallback.ok(),
            Some(PathBuf::from("C:/Users/demo").join("AppData").join("Local"))
        );
        assert!(matches!(
            platform_data_root(|_| None),
            Err(DirectoryError::ApplicationDataUnavailable)
        ));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn unix_root_prefers_xdg_data_home_and_falls_back_to_home() {
        let preferred = platform_data_root(|name| match name {
            "XDG_DATA_HOME" => Some(OsString::from("/home/demo/.local/state")),
            _ => None,
        });
        assert_eq!(
            preferred.ok().as_deref(),
            Some(Path::new("/home/demo/.local/state"))
        );
        let fallback = platform_data_root(|name| match name {
            "HOME" => Some(OsString::from("/home/demo")),
            _ => None,
        });
        assert_eq!(
            fallback.ok(),
            Some(PathBuf::from("/home/demo").join(".local").join("share"))
        );
        assert!(matches!(
            platform_data_root(|_| None),
            Err(DirectoryError::ApplicationDataUnavailable)
        ));
    }
}
