//! Instalaciones desechables con la custodia nativa ya aplicada.
//!
//! En Windows un directorio temporal hereda concesiones de grupo del perfil, que
//! la política de producto rechaza por diseño. Las pruebas deben partir de una
//! ruta que cumpla las mismas reglas que crea el producto, no de una ruta que el
//! producto rechazaría.

use std::path::{Path, PathBuf};

/// Instalación temporal cuyo directorio ya cumple la custodia de la plataforma.
pub(crate) struct Fixture {
    /// Mantiene vivo el directorio temporal durante toda la prueba.
    _root: tempfile::TempDir,
    path: PathBuf,
}

impl Fixture {
    /// Crea la instalación de prueba dentro de un directorio temporal nuevo.
    pub(crate) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("com.hiqnet.pos");
        create_private(&path)?;
        Ok(Self { _root: root, path })
    }

    /// Directorio privado que se entrega a `LocalState::open_installation`.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(unix)]
fn create_private(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(windows)]
fn create_private(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    pos_windows_security::create_private_directory(path)?;
    Ok(())
}
