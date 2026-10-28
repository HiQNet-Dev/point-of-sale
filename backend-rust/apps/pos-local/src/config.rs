//! Configuración local explícita, sin valores por defecto de producto.

use std::{env, ffi::OsString, fmt, io, path::Path, path::PathBuf, time::Duration};

/// Configuración validada del proceso local.
///
/// No implementa `Debug` para evitar volcar rutas al registrar diagnósticos.
/// El timeout es una decisión explícita de laboratorio, no un valor de producto.
pub struct Config {
    db_path: PathBuf,
    busy_timeout: Duration,
}

impl Config {
    /// Lee `POS_LOCAL_DB_PATH` y `POS_LOCAL_BUSY_TIMEOUT_MS`, ambas obligatorias.
    ///
    /// # Errors
    /// Devuelve [`ConfigError`] sin conservar valores de entorno rechazados.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_values(
            env::var_os("POS_LOCAL_DB_PATH"),
            env::var_os("POS_LOCAL_BUSY_TIMEOUT_MS"),
        )
    }

    /// Valida valores de entorno sin modificar el entorno global del proceso.
    ///
    /// La ruta debe ser Unicode, de archivo local y no una URL/URI. Se rechazan
    /// rutas de red UNC, controles, espacios exteriores y los caracteres
    /// reservados `:`, `?`, `#` y `\`. Un archivo nuevo es válido; un directorio
    /// o archivo especial existente no lo es. Esta validación no es un sandbox
    /// ni sustituye los permisos del sistema de archivos.
    ///
    /// El timeout debe contener solo dígitos ASCII y representar `1..=60000`
    /// milisegundos, sin valor implícito de producto.
    ///
    /// # Errors
    /// Devuelve [`ConfigError`] por ausencia, sintaxis o rango no válidos.
    pub fn from_values(
        db_path: Option<OsString>,
        busy_timeout_ms: Option<OsString>,
    ) -> Result<Self, ConfigError> {
        let raw_path = db_path.ok_or(ConfigError::MissingDbPath)?;
        let path = raw_path.to_str().ok_or(ConfigError::InvalidDbPath)?;
        let leaf = path.rsplit('/').next();
        if path.is_empty()
            || path.trim() != path
            || path.chars().any(char::is_control)
            || path.contains([':', '?', '#', '\\'])
            || path.starts_with("//")
            || matches!(leaf, None | Some("" | "." | ".."))
        {
            return Err(ConfigError::InvalidDbPath);
        }
        match Path::new(path).metadata() {
            Ok(metadata) if !metadata.is_file() => return Err(ConfigError::InvalidDbPath),
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(ConfigError::InvalidDbPath);
            }
            _ => {}
        }
        let raw_timeout = busy_timeout_ms.ok_or(ConfigError::MissingBusyTimeout)?;
        let timeout = raw_timeout
            .to_str()
            .ok_or(ConfigError::InvalidBusyTimeout)?;
        if timeout.is_empty() || !timeout.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ConfigError::InvalidBusyTimeout);
        }
        let timeout: u64 = timeout
            .parse()
            .map_err(|_| ConfigError::InvalidBusyTimeout)?;
        if !(1..=60_000).contains(&timeout) {
            return Err(ConfigError::InvalidBusyTimeout);
        }
        Ok(Self {
            db_path: PathBuf::from(path),
            busy_timeout: Duration::from_millis(timeout),
        })
    }

    /// Devuelve la ruta validada del archivo de base de datos.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Devuelve el timeout explícito de espera ante bloqueos SQLite.
    pub fn busy_timeout(&self) -> Duration {
        self.busy_timeout
    }
}

/// Fallos de configuración sin rutas ni valores de entorno en sus datos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// Falta el valor obligatorio de la ruta.
    MissingDbPath,
    /// La ruta no cumple el contrato de archivo local.
    InvalidDbPath,
    /// Falta el timeout obligatorio del laboratorio.
    MissingBusyTimeout,
    /// El timeout no es un entero ASCII de `1..=60000` milisegundos.
    InvalidBusyTimeout,
}

impl ConfigError {
    /// Devuelve un código estático seguro para telemetría estructurada.
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingDbPath => "missing_db_path",
            Self::InvalidDbPath => "invalid_db_path",
            Self::MissingBusyTimeout => "missing_busy_timeout",
            Self::InvalidBusyTimeout => "invalid_busy_timeout",
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingDbPath => "falta la ruta obligatoria de base de datos",
            Self::InvalidDbPath => "se requiere una ruta de archivo local válida",
            Self::MissingBusyTimeout => "falta el timeout explícito de laboratorio",
            Self::InvalidBusyTimeout => "el timeout debe ser un entero de 1 a 60000 milisegundos",
        })
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
