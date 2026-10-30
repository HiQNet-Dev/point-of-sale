//! Composition root de la aplicación de escritorio Local.
//!
//! Dos modos explícitos, sin rutas implícitas:
//!
//! - **Laboratorio** (`--synthetic-lab`): base efímera, fixtures sintéticas y
//!   lectura de catálogo. Nunca es identidad de producto.
//! - **Instalación** (`--installation <directorio>`): archivo SQLite persistente
//!   dentro del directorio indicado, migraciones aprobadas y estado de
//!   instalación. No crea sesiones, operadores ni credenciales sintéticas y no
//!   expone operaciones de negocio: las decisiones físicas siguen pendientes.
//!
//! Ningún modo abre HTTP, lee secretos del entorno ni acepta rutas, organización
//! o actor desde JavaScript.

use std::{error::Error, fmt, path::Path, time::Duration};

use pos_application::{
    catalog::CatalogPage,
    installation::{
        ApprovedSchema, InstallationError, InstallationReport, InstallationState,
        prepare_installation,
    },
    organization::list_authorized_branches,
};
use pos_core::identity::VerifiedPrincipal;
use pos_storage_sqlite::{
    SqliteOrganizationBranchStore, SqliteUnitOfWork, approved_schema_version,
    catalog::SqliteCatalogApplication, checkpoint, installation::SqliteInstallationSchema,
};
use sqlx::SqlitePool;
use tempfile::TempDir;

use crate::dto::{
    BranchDto, CategoryDto, InstallationStatusDto, IpcError, PageRequest, ProductDto,
};

/// Nombre fijo del archivo dentro del directorio de instalación.
pub const DATABASE_FILE_NAME: &str = "hiqnet-pos.sqlite";

/// Espera ante bloqueos mientras la política de producto no esté aprobada.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Estado compartido por los handlers IPC de ambos modos.
pub struct LocalState {
    mode: Mode,
}

enum Mode {
    Lab(LabInstallation),
    Product(ProductInstallation),
}

struct LabInstallation {
    pool: SqlitePool,
    _directory: TempDir,
}

struct ProductInstallation {
    pool: SqlitePool,
    report: InstallationReport,
}

/// Fallos de arranque sin SQL, credenciales ni contenidos de fila.
#[derive(Debug)]
pub enum StartupError {
    /// El directorio de instalación no es utilizable.
    InvalidInstallationDirectory,
    /// El archivo no pudo abrirse con los PRAGMA exigidos.
    Storage,
    /// El historial de migraciones no permite operar.
    Installation(InstallationError),
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInstallationDirectory => {
                formatter.write_str("directorio de instalación inválido")
            }
            Self::Storage => formatter.write_str("no se pudo abrir el archivo SQLite local"),
            Self::Installation(error) => write!(formatter, "instalación rechazada: {error}"),
        }
    }
}

impl Error for StartupError {}

impl From<InstallationError> for StartupError {
    fn from(error: InstallationError) -> Self {
        Self::Installation(error)
    }
}

impl LocalState {
    /// Instalación sintética efímera con los adaptadores existentes.
    ///
    /// # Errors
    /// Devuelve [`StartupError`] si el archivo temporal no puede abrirse o el
    /// escenario de laboratorio no se confirma.
    pub async fn synthetic() -> Result<Self, StartupError> {
        let directory = tempfile::tempdir().map_err(|_| StartupError::Storage)?;
        let pool = pos_storage_sqlite::open(&directory.path().join("desktop.sqlite"), BUSY_TIMEOUT)
            .await
            .map_err(|_| StartupError::Storage)?;
        pos_lab::run(&pool)
            .await
            .map_err(|_| StartupError::Storage)?;
        Ok(Self {
            mode: Mode::Lab(LabInstallation {
                pool,
                _directory: directory,
            }),
        })
    }

    /// Abre o crea la instalación persistente del directorio indicado.
    ///
    /// El archivo se crea si no existe; nunca se borra ni se reemplaza. El
    /// directorio debe existir o poder crearse, y el archivo resultante debe
    /// tener un historial compatible con este build.
    ///
    /// # Errors
    /// Devuelve [`StartupError`] si el directorio es inválido, si el archivo no
    /// puede abrirse o si el esquema pertenece a un binario más reciente.
    pub async fn open_installation(directory: &Path) -> Result<Self, StartupError> {
        if directory.as_os_str().is_empty() {
            return Err(StartupError::InvalidInstallationDirectory);
        }
        if directory.exists() && !directory.is_dir() {
            return Err(StartupError::InvalidInstallationDirectory);
        }
        std::fs::create_dir_all(directory)
            .map_err(|_| StartupError::InvalidInstallationDirectory)?;
        let database = directory.join(DATABASE_FILE_NAME);
        if database.is_dir() {
            return Err(StartupError::InvalidInstallationDirectory);
        }
        let existed = database.exists();
        let pool = pos_storage_sqlite::open(&database, BUSY_TIMEOUT)
            .await
            .map_err(|_| StartupError::Storage)?;
        let schema = SqliteInstallationSchema::new(pool.clone(), existed);
        let approved = ApprovedSchema::new(approved_schema_version())?;
        let report = match prepare_installation(&schema, &schema, approved).await {
            Ok(report) => report,
            Err(error) => {
                pool.close().await;
                return Err(StartupError::Installation(error));
            }
        };
        Ok(Self {
            mode: Mode::Product(ProductInstallation { pool, report }),
        })
    }

    /// Cierra las conexiones; checkpoint previo solo en la instalación persistente.
    pub async fn shutdown(&self) {
        match &self.mode {
            Mode::Lab(lab) => lab.pool.close().await,
            Mode::Product(product) => {
                // Un cierre limpio deja el WAL integrado en el archivo principal.
                let _ = checkpoint(&product.pool).await;
                product.pool.close().await;
            }
        }
    }

    fn lab(&self) -> Result<&LabInstallation, IpcError> {
        match &self.mode {
            Mode::Lab(lab) => Ok(lab),
            // El modo producto no sirve datos sintéticos ni consulta tablas no aprobadas.
            Mode::Product(_) => Err(IpcError::OperationNotAvailable),
        }
    }

    /// Estado de la instalación persistente; el laboratorio no lo expone.
    pub fn installation_status(&self) -> Result<InstallationStatusDto, IpcError> {
        match &self.mode {
            Mode::Lab(_) => Err(IpcError::OperationNotAvailable),
            Mode::Product(product) => Ok(InstallationStatusDto {
                state: match product.report.state {
                    InstallationState::FirstInstall => "FIRST_INSTALL",
                    InstallationState::Reopened => "REOPENED",
                }
                .to_owned(),
                // Las versiones de migración son enteros pequeños y acotados;
                // Specta prohíbe i64 precisamente para no perder precisión en JS.
                schema_version: u32::try_from(product.report.schema_version)
                    .map_err(|_| IpcError::InternalError)?,
                applied_migrations: product.report.applied_migrations,
            }),
        }
    }

    async fn principal(&self) -> Result<VerifiedPrincipal, IpcError> {
        let lab = self.lab()?;
        // La sesión es una fixture declarada. Se consulta de nuevo en cada llamada:
        // revocación, estados y permisos siguen bajo autoridad de SQLite.
        pos_lab::authenticate_candidate_session(&lab.pool, None)
            .await
            .map_err(|_| IpcError::Unauthorized)
    }

    /// Ejecuta la consulta Application; no filtra mediante JavaScript.
    pub async fn branches(&self) -> Result<Vec<BranchDto>, IpcError> {
        let principal = self.principal().await?;
        let reader = SqliteOrganizationBranchStore::new(self.pool()?.clone());
        let branches = list_authorized_branches(&reader, &principal).await?;
        Ok(branches
            .into_iter()
            .map(|branch| BranchDto {
                id: branch.id().as_str().into(),
                code: branch.code().as_str().into(),
                name: branch.name().as_str().into(),
                status: branch.status().as_str().into(),
            })
            .collect())
    }

    /// Lista categorías scoped usando el caso de uso existente.
    pub async fn categories(&self, page: PageRequest) -> Result<Vec<CategoryDto>, IpcError> {
        let principal = self.principal().await?;
        let catalog = SqliteCatalogApplication::new(SqliteUnitOfWork::new(self.pool()?.clone()));
        let categories = catalog
            .list_categories(
                &principal,
                None,
                CatalogPage {
                    limit: page.limit,
                    offset: page.offset,
                },
            )
            .await?;
        Ok(categories
            .into_iter()
            .map(|category| CategoryDto {
                id: category.id().as_str().into(),
                code: category.code().as_str().into(),
                name: category.name().as_str().into(),
                version: category.revision().to_string(),
            })
            .collect())
    }

    /// Lista productos sin convertir sus precios a float ni a unidades menores.
    pub async fn products(&self, page: PageRequest) -> Result<Vec<ProductDto>, IpcError> {
        let principal = self.principal().await?;
        let catalog = SqliteCatalogApplication::new(SqliteUnitOfWork::new(self.pool()?.clone()));
        let products = catalog
            .list_products(
                &principal,
                None,
                None,
                CatalogPage {
                    limit: page.limit,
                    offset: page.offset,
                },
            )
            .await?;
        Ok(products
            .into_iter()
            .map(|product| ProductDto {
                id: product.id().as_str().into(),
                sku: product.sku().as_str().into(),
                name: product.name().as_str().into(),
                unit_price: product.unit_price().to_string(),
                category_id: product.category_id().map(|id| id.as_str().into()),
                version: product.revision().to_string(),
            })
            .collect())
    }

    fn pool(&self) -> Result<&SqlitePool, IpcError> {
        Ok(&self.lab()?.pool)
    }
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;
