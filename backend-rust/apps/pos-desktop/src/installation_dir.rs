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
//! candado de instancia única. La ruta por plataforma (`app_data_dir`) depende de
//! D-INST y no se resuelve aquí.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Nombre fijo del archivo dentro del directorio de instalación.
pub const DATABASE_FILE_NAME: &str = "hiqnet-pos.sqlite";

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
            Self::Unavailable => "el directorio de instalación no está disponible",
        })
    }
}

impl std::error::Error for DirectoryError {}

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
}
