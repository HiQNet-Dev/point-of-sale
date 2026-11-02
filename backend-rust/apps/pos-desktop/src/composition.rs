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

use std::{
    error::Error,
    fmt,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use pos_application::{
    catalog::{CatalogPage, NewProductRequest},
    installation::{
        ApprovedSchema, InstallationError, InstallationReport, InstallationState,
        prepare_installation,
    },
    organization::list_authorized_branches,
};
use pos_core::{
    audit::AuditOccurredAt,
    identity::VerifiedPrincipal,
    organization_branch::{ApplicationError, EntityId, IdentityGenerator},
};
use pos_storage_sqlite::{
    SqliteOrganizationBranchStore, SqliteUnitOfWork, approved_schema_version,
    catalog::SqliteCatalogApplication, checkpoint, installation::SqliteInstallationSchema,
};
use sqlx::SqlitePool;
use tempfile::TempDir;

use crate::{
    dto::{
        BranchDto, CategoryDto, CreateProductRequest, CreatedProductDto, InstallationStatusDto,
        IpcError, PageRequest, ProductDto,
    },
    installation_dir::InstallationDirectory,
};

/// Nombre del archivo, expuesto solo a las pruebas de composición.
#[cfg(test)]
pub use crate::installation_dir::DATABASE_FILE_NAME;

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

pub(crate) struct LabInstallation {
    pub(crate) pool: SqlitePool,
    /// Contador de identidades del proceso; evita reutilizar UUID entre escrituras.
    pub(crate) next_identity: AtomicU64,
    _directory: TempDir,
}

pub(crate) struct ProductInstallation {
    pub(crate) pool: SqlitePool,
    pub(crate) report: InstallationReport,
    /// Directorio privado; el bootstrap lee aquí la autorización nativa.
    pub(crate) directory: std::path::PathBuf,
    installation_lock: std::fs::File,
    /// Handle de sesión en memoria nativa; nunca se persiste ni llega al
    /// renderer. Se libera al cerrar la aplicación.
    pub(crate) session: tokio::sync::Mutex<Option<String>>,
    /// Sucursal elegida por la persona autenticada.
    ///
    /// Vive en memoria nativa junto al handle: los comandos operativos la leen
    /// de aquí en lugar de aceptarla en cada petición. Se limpia al iniciar o
    /// cerrar sesión.
    pub(crate) selected_branch: tokio::sync::Mutex<Option<EntityId>>,
}

/// Fallos de arranque sin SQL, credenciales ni contenidos de fila.
#[derive(Debug)]
pub enum StartupError {
    /// El directorio de instalación no es utilizable.
    InvalidInstallationDirectory,
    /// Otro runtime desktop mantiene abierta esta instalación.
    InstallationInUse,
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
            Self::InstallationInUse => formatter.write_str("la instalación ya está abierta"),
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

impl From<crate::installation_dir::DirectoryError> for StartupError {
    fn from(error: crate::installation_dir::DirectoryError) -> Self {
        match error {
            crate::installation_dir::DirectoryError::InUse => Self::InstallationInUse,
            _ => Self::InvalidInstallationDirectory,
        }
    }
}

/// Generador de identidades del laboratorio, ligado al estado del proceso.
struct DesktopIdentities<'a> {
    counter: &'a AtomicU64,
}

impl IdentityGenerator for DesktopIdentities<'_> {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        let value = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        EntityId::parse(&format!("00000000-0000-0000-0000-{value:012x}"))
            .map_err(|_| ApplicationError::Storage)
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
                next_identity: AtomicU64::new(0x9000_0000),
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
        // El directorio se valida antes de abrir SQLite: enlaces y permisos
        // compartidos se rechazan en el entrypoint, no en la capa de datos.
        let directory = InstallationDirectory::resolve(directory)?;
        let installation_lock = directory.acquire_lock()?;
        let database = directory.database_file();
        let existed = database.exists();
        let installation_directory = directory.path().to_path_buf();
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
            mode: Mode::Product(ProductInstallation {
                pool,
                report,
                directory: installation_directory,
                installation_lock,
                session: tokio::sync::Mutex::new(None),
                selected_branch: tokio::sync::Mutex::new(None),
            }),
        })
    }

    /// Cierra las conexiones; checkpoint previo solo en la instalación persistente.
    pub async fn shutdown(&self) {
        match &self.mode {
            Mode::Lab(lab) => lab.pool.close().await,
            Mode::Product(product) => {
                // El handle sale de memoria al cerrar: no se persiste en ningún sitio.
                *product.session.lock().await = None;
                *product.selected_branch.lock().await = None;
                // Un cierre limpio deja el WAL integrado en el archivo principal.
                let _ = checkpoint(&product.pool).await;
                product.pool.close().await;
                // Nunca se libera mientras SQLite siga abierto; en caso de fallo,
                // el SO lo liberará al destruir el estado o terminar el proceso.
                let _ = product.installation_lock.unlock();
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

    /// Crea un producto con el permiso `product.create` del principal verificado.
    ///
    /// # Solo laboratorio
    ///
    /// Es la primera escritura auditada del transporte. Se expone únicamente en
    /// el modo sintético mientras el esquema de catálogo, la identidad comercial
    /// y la auditoría productiva sigan pendientes de aprobación. El actor y la
    /// organización se derivan del principal; el request no puede suministrarlos.
    ///
    /// # Errors
    /// Devuelve [`IpcError::OperationNotAvailable`] fuera del laboratorio,
    /// [`IpcError::Forbidden`] sin permiso, [`IpcError::ValidationError`] si el
    /// SKU, el nombre o el precio no cumplen el contrato, y
    /// [`IpcError::Conflict`] si el SKU o el código de barras ya existen.
    pub async fn create_product(
        &self,
        request: CreateProductRequest,
    ) -> Result<CreatedProductDto, IpcError> {
        let lab = self.lab()?;
        let principal = self.principal().await?;
        let occurred_at =
            AuditOccurredAt::new(pos_lab::LAB_OCCURRED_AT).map_err(|_| IpcError::InternalError)?;
        let catalog = SqliteCatalogApplication::new(SqliteUnitOfWork::new(lab.pool.clone()));
        let mut identities = DesktopIdentities {
            counter: &lab.next_identity,
        };
        let product = catalog
            .create_product(
                &principal,
                &mut identities,
                NewProductRequest {
                    sku: request.sku,
                    name: request.name,
                    unit_price: request.unit_price,
                    barcode: request.barcode,
                    description: request.description,
                    category_id: request
                        .category_id
                        .map(|value| EntityId::parse(&value))
                        .transpose()
                        .map_err(|_| IpcError::ValidationError)?,
                },
                &occurred_at,
            )
            .await?;
        Ok(CreatedProductDto {
            id: product.id().as_str().into(),
            sku: product.sku().as_str().into(),
            name: product.name().as_str().into(),
            unit_price: product.unit_price().to_string(),
            category_id: product.category_id().map(|id| id.as_str().into()),
            version: product.revision().to_string(),
        })
    }

    fn pool(&self) -> Result<&SqlitePool, IpcError> {
        Ok(&self.lab()?.pool)
    }

    /// Instalación persistente; el laboratorio no tiene identidad de producto.
    pub(crate) fn product_installation(&self) -> Result<&ProductInstallation, IpcError> {
        match &self.mode {
            Mode::Product(product) => Ok(product),
            Mode::Lab(_) => Err(IpcError::OperationNotAvailable),
        }
    }
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;
