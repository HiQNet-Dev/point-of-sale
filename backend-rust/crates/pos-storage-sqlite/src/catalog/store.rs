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

use pos_core::{
    audit::AuditOccurredAt,
    catalog::{Barcode, CatalogCode, Category, Description, Product, Sku, UnitPrice},
    identity::UtcSeconds,
    organization_branch::{ApplicationError, DisplayName, EntityId, Status},
};

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork};

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

const CATEGORY_CODE_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM category WHERE organization_id = ? AND code = ?)";
const SELECT_CATEGORY_VERSION: &str =
    "SELECT version FROM category WHERE organization_id = ? AND id = ?";

const PRODUCT_SKU_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM product WHERE organization_id = ? AND sku = ?)";
const PRODUCT_BARCODE_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM product WHERE organization_id = ? AND barcode = ?)";
const SELECT_PRODUCT_VERSION: &str =
    "SELECT version FROM product WHERE organization_id = ? AND id = ?";

#[derive(sqlx::FromRow)]
struct CategoryRow {
    id: String,
    organization_id: String,
    code: String,
    name: String,
    status: String,
    version: i64,
}

impl CategoryRow {
    fn into_domain(self) -> Result<Category, CatalogStorageError> {
        Ok(Category::restore(
            EntityId::parse(&self.id).map_err(|_| CatalogStorageError::InvalidRow)?,
            EntityId::parse(&self.organization_id).map_err(|_| CatalogStorageError::InvalidRow)?,
            CatalogCode::restore(&self.code).map_err(|_| CatalogStorageError::InvalidRow)?,
            DisplayName::new(&self.name).map_err(|_| CatalogStorageError::InvalidRow)?,
            Status::parse(&self.status).map_err(|_| CatalogStorageError::InvalidRow)?,
            revision_to_u64(self.version)?,
        ))
    }
}

#[derive(sqlx::FromRow)]
struct ProductRow {
    id: String,
    organization_id: String,
    category_id: Option<String>,
    sku: String,
    barcode: Option<String>,
    name: String,
    description: Option<String>,
    unit_price: String,
    status: String,
    version: i64,
}

impl ProductRow {
    fn into_domain(self) -> Result<Product, CatalogStorageError> {
        let category_id = match self.category_id {
            Some(value) => {
                Some(EntityId::parse(&value).map_err(|_| CatalogStorageError::InvalidRow)?)
            }
            None => None,
        };
        Ok(Product::restore(
            EntityId::parse(&self.id).map_err(|_| CatalogStorageError::InvalidRow)?,
            EntityId::parse(&self.organization_id).map_err(|_| CatalogStorageError::InvalidRow)?,
            category_id,
            Sku::restore(&self.sku).map_err(|_| CatalogStorageError::InvalidRow)?,
            self.barcode
                .as_deref()
                .map(Barcode::restore)
                .transpose()
                .map_err(|_| CatalogStorageError::InvalidRow)?,
            DisplayName::new(&self.name).map_err(|_| CatalogStorageError::InvalidRow)?,
            self.description
                .as_deref()
                .map(Description::restore)
                .transpose()
                .map_err(|_| CatalogStorageError::InvalidRow)?,
            UnitPrice::parse(&self.unit_price).map_err(|_| CatalogStorageError::InvalidRow)?,
            Status::parse(&self.status).map_err(|_| CatalogStorageError::InvalidRow)?,
            revision_to_u64(self.version)?,
        ))
    }
}

fn revision_to_u64(value: i64) -> Result<u64, CatalogStorageError> {
    u64::try_from(value).map_err(|_| CatalogStorageError::InvalidRow)
}

fn revision_to_i64(value: u64) -> Result<i64, CatalogStorageError> {
    i64::try_from(value).map_err(|_| CatalogStorageError::InvalidRow)
}

/// Traduce el error SQLite sin confiar en la ausencia de mensaje.
fn map_write_error(error: &sqlx::Error) -> CatalogStorageError {
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

impl SqliteTransaction {
    /// Busca una categoría dentro de su organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla o la fila es inválida.
    pub async fn get_category(
        &mut self,
        organization_id: &EntityId,
        category_id: &EntityId,
    ) -> Result<Option<Category>, CatalogStorageError> {
        let row = sqlx::query_as::<_, CategoryRow>(SELECT_CATEGORY)
            .bind(organization_id.as_str())
            .bind(category_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        row.map(CategoryRow::into_domain).transpose()
    }

    /// Indica si el código ya está ocupado dentro de la organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla.
    pub async fn category_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> Result<bool, CatalogStorageError> {
        let exists: i64 = sqlx::query_scalar(CATEGORY_CODE_EXISTS)
            .bind(organization_id.as_str())
            .bind(code.as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        Ok(exists != 0)
    }

    /// Inserta una categoría en la transacción abierta.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError::DuplicateCategoryCode`] o
    /// [`CatalogStorageError::ForeignKey`].
    pub async fn insert_category(
        &mut self,
        category: &Category,
        occurred_at: &str,
    ) -> Result<(), CatalogStorageError> {
        sqlx::query(INSERT_CATEGORY)
            .bind(category.id().as_str())
            .bind(category.organization_id().as_str())
            .bind(category.code().as_str())
            .bind(category.name().as_str())
            .bind(category.status().as_str())
            .bind(revision_to_i64(category.revision())?)
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?;
        Ok(())
    }

    /// Actualiza el nombre y el estado con CAS scoped por organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla.
    pub async fn update_category_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        category: &Category,
        occurred_at: &str,
    ) -> Result<CasOutcome, CatalogStorageError> {
        let affected = sqlx::query(UPDATE_CATEGORY_CAS)
            .bind(category.name().as_str())
            .bind(category.status().as_str())
            .bind(revision_to_i64(category.revision())?)
            .bind(occurred_at)
            .bind(organization_id.as_str())
            .bind(category.id().as_str())
            .bind(revision_to_i64(previous_revision)?)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?
            .rows_affected();
        if affected != 0 {
            return Ok(CasOutcome::Updated);
        }
        let exists: Option<i64> = sqlx::query_scalar(SELECT_CATEGORY_VERSION)
            .bind(organization_id.as_str())
            .bind(category.id().as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        Ok(if exists.is_some() {
            CasOutcome::Conflict
        } else {
            CasOutcome::Missing
        })
    }

    /// Lista categorías de una organización, opcionalmente por estado.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla o una fila es inválida.
    pub async fn list_categories(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Category>, CatalogStorageError> {
        let status = status.map(Status::as_str);
        let rows = sqlx::query_as::<_, CategoryRow>(LIST_CATEGORIES)
            .bind(organization_id.as_str())
            .bind(status)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        rows.into_iter().map(CategoryRow::into_domain).collect()
    }

    /// Busca un producto dentro de su organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla o la fila es inválida.
    pub async fn get_product(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> Result<Option<Product>, CatalogStorageError> {
        let row = sqlx::query_as::<_, ProductRow>(SELECT_PRODUCT)
            .bind(organization_id.as_str())
            .bind(product_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        row.map(ProductRow::into_domain).transpose()
    }

    /// Indica si el SKU ya está ocupado dentro de la organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla.
    pub async fn product_sku_exists(
        &mut self,
        organization_id: &EntityId,
        sku: &Sku,
    ) -> Result<bool, CatalogStorageError> {
        let exists: i64 = sqlx::query_scalar(PRODUCT_SKU_EXISTS)
            .bind(organization_id.as_str())
            .bind(sku.as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        Ok(exists != 0)
    }

    /// Indica si el código de barras ya está ocupado dentro de la organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla.
    pub async fn product_barcode_exists(
        &mut self,
        organization_id: &EntityId,
        barcode: &Barcode,
    ) -> Result<bool, CatalogStorageError> {
        let exists: i64 = sqlx::query_scalar(PRODUCT_BARCODE_EXISTS)
            .bind(organization_id.as_str())
            .bind(barcode.as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        Ok(exists != 0)
    }

    /// Inserta un producto en la transacción abierta.
    ///
    /// El precio se escribe en su forma canónica escala 4, que es la que el
    /// `CHECK` físico acepta.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite rechaza la fila.
    pub async fn insert_product(
        &mut self,
        product: &Product,
        occurred_at: &str,
    ) -> Result<(), CatalogStorageError> {
        sqlx::query(INSERT_PRODUCT)
            .bind(product.id().as_str())
            .bind(product.organization_id().as_str())
            .bind(product.category_id().map(EntityId::as_str))
            .bind(product.sku().as_str())
            .bind(product.barcode().map(Barcode::as_str))
            .bind(product.name().as_str())
            .bind(product.description().map(Description::as_str))
            .bind(product.unit_price().to_string())
            .bind(product.status().as_str())
            .bind(revision_to_i64(product.revision())?)
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?;
        Ok(())
    }

    /// Actualiza un producto completo con CAS scoped por organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite rechaza la fila.
    pub async fn update_product_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        product: &Product,
        occurred_at: &str,
    ) -> Result<CasOutcome, CatalogStorageError> {
        let affected = sqlx::query(UPDATE_PRODUCT_CAS)
            .bind(product.category_id().map(EntityId::as_str))
            .bind(product.barcode().map(Barcode::as_str))
            .bind(product.name().as_str())
            .bind(product.description().map(Description::as_str))
            .bind(product.unit_price().to_string())
            .bind(product.status().as_str())
            .bind(revision_to_i64(product.revision())?)
            .bind(occurred_at)
            .bind(organization_id.as_str())
            .bind(product.id().as_str())
            .bind(revision_to_i64(previous_revision)?)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?
            .rows_affected();
        if affected != 0 {
            return Ok(CasOutcome::Updated);
        }
        let exists: Option<i64> = sqlx::query_scalar(SELECT_PRODUCT_VERSION)
            .bind(organization_id.as_str())
            .bind(product.id().as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        Ok(if exists.is_some() {
            CasOutcome::Conflict
        } else {
            CasOutcome::Missing
        })
    }

    /// Lista productos de una organización con filtros scoped y paginación.
    ///
    /// # Errors
    /// Devuelve [`CatalogStorageError`] si SQLite falla o una fila es inválida.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_products(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        category_id: Option<&EntityId>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Product>, CatalogStorageError> {
        let status = status.map(Status::as_str);
        let category = category_id.map(EntityId::as_str);
        let rows = sqlx::query_as::<_, ProductRow>(LIST_PRODUCTS)
            .bind(organization_id.as_str())
            .bind(status)
            .bind(status)
            .bind(category)
            .bind(category)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| CatalogStorageError::Database)?;
        rows.into_iter().map(ProductRow::into_domain).collect()
    }
}

/// Tamaño máximo de página admitido por la capa de aplicación.
///
/// El contrato HTTP aún no existe; este límite evita que un transporte futuro
/// solicite el catálogo completo en una sola respuesta.
pub const MAX_PAGE_SIZE: u32 = 200;

fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}

pub use pos_application::catalog::{
    CatalogApplication, CatalogPage, CatalogTransaction, CatalogUnitOfWork, CategoryUpdateRequest,
    NewCategoryRequest, NewProductRequest, ProductUpdateRequest,
};

use super::queries::*;

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

impl CatalogTransaction for SqliteTransaction {
    fn get_category(
        &mut self,
        organization_id: &EntityId,
        category_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Category>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let category_id = category_id.clone();
        async move {
            self.get_category(&organization_id, &category_id)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn category_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let code = code.clone();
        async move {
            self.category_code_exists(&organization_id, &code)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn insert_category(
        &mut self,
        category: &Category,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let category = category.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.insert_category(&category, &occurred_at)
                .await
                .map_err(|error| catalog_write_error(&error))
        }
    }

    fn update_category_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        category: &Category,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let category = category.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.update_category_cas(&organization_id, previous_revision, &category, &occurred_at)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn list_categories(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Category>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        async move {
            self.list_categories(&organization_id, status, limit, offset)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn get_product(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Product>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let product_id = product_id.clone();
        async move {
            self.get_product(&organization_id, &product_id)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn product_sku_exists(
        &mut self,
        organization_id: &EntityId,
        sku: &Sku,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let sku = sku.clone();
        async move {
            self.product_sku_exists(&organization_id, &sku)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn product_barcode_exists(
        &mut self,
        organization_id: &EntityId,
        barcode: &Barcode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let barcode = barcode.clone();
        async move {
            self.product_barcode_exists(&organization_id, &barcode)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn insert_product(
        &mut self,
        product: &Product,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let product = product.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.insert_product(&product, &occurred_at)
                .await
                .map_err(|error| catalog_write_error(&error))
        }
    }

    fn update_product_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        product: &Product,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let product = product.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.update_product_cas(&organization_id, previous_revision, &product, &occurred_at)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn list_products(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        category_id: Option<&EntityId>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Product>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let category_id = category_id.cloned();
        async move {
            self.list_products(
                &organization_id,
                status,
                category_id.as_ref(),
                limit,
                offset,
            )
            .await
            .map_err(|_| ApplicationError::Storage)
        }
    }

    fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send {
        let occurred_at = occurred_at.clone();
        async move {
            self.candidate_utc_seconds(&occurred_at)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }
    fn get_organization(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<
        Output = Result<Option<pos_core::organization_branch::Organization>, ApplicationError>,
    > + Send {
        let organization_id = organization_id.clone();
        async move {
            self.get_organization(&organization_id)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }
}

fn catalog_write_error(error: &CatalogStorageError) -> ApplicationError {
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
