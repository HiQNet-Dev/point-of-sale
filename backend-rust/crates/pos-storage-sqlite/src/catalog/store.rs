//! Adaptador SQLite del catálogo organizacional y sus casos de uso.
//!
//! **No productivo todavía.** Las tablas viven en
//! `backend-rust/proposals/local-catalog/0008_catalog.proposed.sql`, fuera del
//! migrador, y `pos-local` no las carga. El módulo existe para que la promoción
//! consista en mover la propuesta, no en reescribir la persistencia.
//!
//! # Frontera transaccional
//!
//! Toda mutación pasa por [`SqliteUnitOfWork`]: una sola conexión con
//! `BEGIN IMMEDIATE`, el `INSERT`/`UPDATE` scoped, la escritura de auditoría y un
//! único `commit`. Si la auditoría falla, la mutación se revierte con ella. No se
//! abre ninguna conexión adicional mientras la unidad de trabajo está activa.
//!
//! # Autorización
//!
//! Los casos de uso reciben un [`VerifiedPrincipal`] y resuelven el permiso con
//! [`VerifiedPrincipal::authorize`], que es deny-by-default y exige el scope
//! exacto. El catálogo es de la ORGANIZACIÓN, así que se autoriza con un scope
//! organizacional (`branch_id = None`); un principal resuelto para una sucursal
//! no lo autoriza.
//!
//! # Ownership
//!
//! Toda lectura y escritura incluye `organization_id` en el `WHERE`. Una fila de
//! otra organización se comporta como inexistente. Además, el esquema impone la
//! FK compuesta `(category_id, organization_id)`: la barrera no depende de que
//! el adaptador se acuerde de comprobarla.

use pos_core::organization_branch::ApplicationError;

use crate::prelude::transaction_error;
use crate::{SqliteTransaction, SqliteUnitOfWork};

/// Error tipado del adaptador de catálogo, sin valores de entrada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogStorageError {
    /// El código de categoría ya existe dentro de la organización.
    DuplicateCategoryCode,
    /// El SKU ya existe dentro de la organización.
    DuplicateProductSku,
    /// El código de barras ya existe dentro de la organización.
    DuplicateProductBarcode,
    /// La organización referenciada no existe.
    ForeignKey,
    /// Una fila persistida incumple los invariantes del dominio.
    InvalidRow,
    /// SQLite rechazó la operación sin exponer SQL ni valores.
    Database,
}

impl std::fmt::Display for CatalogStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::DuplicateCategoryCode => "código de categoría duplicado",
            Self::DuplicateProductSku => "SKU duplicado",
            Self::DuplicateProductBarcode => "código de barras duplicado",
            Self::ForeignKey => "organización referenciada inexistente",
            Self::InvalidRow => "fila de catálogo inválida",
            Self::Database => "operación SQLite de catálogo fallida",
        })
    }
}

impl std::error::Error for CatalogStorageError {}

pub(crate) const CATEGORY_CODE_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM category WHERE organization_id = ? AND code = ?)";
pub(crate) const SELECT_CATEGORY_VERSION: &str =
    "SELECT version FROM category WHERE organization_id = ? AND id = ?";

pub(crate) const PRODUCT_SKU_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM product WHERE organization_id = ? AND sku = ?)";
pub(crate) const PRODUCT_BARCODE_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM product WHERE organization_id = ? AND barcode = ?)";
pub(crate) const SELECT_PRODUCT_VERSION: &str =
    "SELECT version FROM product WHERE organization_id = ? AND id = ?";

pub(crate) fn revision_to_u64(value: i64) -> Result<u64, CatalogStorageError> {
    u64::try_from(value).map_err(|_| CatalogStorageError::InvalidRow)
}

pub(crate) fn revision_to_i64(value: u64) -> Result<i64, CatalogStorageError> {
    i64::try_from(value).map_err(|_| CatalogStorageError::InvalidRow)
}

/// Traduce el error SQLite sin confiar en la ausencia de mensaje.
pub(crate) fn map_write_error(error: &sqlx::Error) -> CatalogStorageError {
    let Some(database) = error.as_database_error() else {
        return CatalogStorageError::Database;
    };
    if database.is_unique_violation() {
        let message = database.message();
        if message.contains("product.sku") {
            return CatalogStorageError::DuplicateProductSku;
        }
        if message.contains("product.barcode") {
            return CatalogStorageError::DuplicateProductBarcode;
        }
        if message.contains("category.code") {
            return CatalogStorageError::DuplicateCategoryCode;
        }
        return CatalogStorageError::Database;
    }
    if database.is_foreign_key_violation() {
        return CatalogStorageError::ForeignKey;
    }
    if database.is_check_violation() {
        return CatalogStorageError::InvalidRow;
    }
    CatalogStorageError::Database
}

/// Tamaño máximo de página admitido por la capa de aplicación.
///
/// El contrato HTTP aún no existe; este límite evita que un transporte futuro
/// solicite el catálogo completo en una sola respuesta.
pub const MAX_PAGE_SIZE: u32 = 200;

pub use pos_application::catalog::{
    CatalogApplication, CatalogPage, CatalogTransaction, CatalogUnitOfWork, CategoryUpdateRequest,
    NewCategoryRequest, NewProductRequest, ProductUpdateRequest,
};

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteCatalogApplication = CatalogApplication<SqliteUnitOfWork>;

impl CatalogUnitOfWork for SqliteUnitOfWork {
    type Transaction = SqliteTransaction;

    async fn begin_transaction(&self) -> Result<SqliteTransaction, ApplicationError> {
        SqliteUnitOfWork::begin(self)
            .await
            .map_err(transaction_error)
    }

    async fn commit_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.commit().await.map_err(transaction_error)
    }

    async fn rollback_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.rollback().await.map_err(transaction_error)
    }
}

pub(crate) fn catalog_write_error(error: &CatalogStorageError) -> ApplicationError {
    match error {
        CatalogStorageError::DuplicateCategoryCode => ApplicationError::CategoryCodeAlreadyExists,
        CatalogStorageError::DuplicateProductSku => ApplicationError::ProductSkuAlreadyExists,
        CatalogStorageError::DuplicateProductBarcode => {
            ApplicationError::ProductBarcodeAlreadyExists
        }
        CatalogStorageError::ForeignKey => ApplicationError::OwnershipMismatch,
        _ => ApplicationError::Storage,
    }
}
