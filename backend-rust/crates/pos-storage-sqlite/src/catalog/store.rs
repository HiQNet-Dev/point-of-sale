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
    audit::{
        AuditAction, AuditEntityType, AuditEventId, AuditEventRecord, AuditMetadata,
        AuditOccurredAt,
    },
    catalog::{
        Barcode, CatalogCode, CatalogError, Category, CategoryPatch, Description, NewCategory,
        NewProduct, PatchField, Product, ProductPatch, Sku, UnitPrice,
    },
    identity::{PermissionCode, PrincipalScope, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, DisplayName, EntityId, IdentityGenerator, Permission, Status,
    },
};
use serde_json::json;

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork, audit_writer::SqliteAuditWriter};

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

const SELECT_CATEGORY: &str = "SELECT id, organization_id, code, name, status, version \
     FROM category WHERE organization_id = ? AND id = ?";
const CATEGORY_CODE_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM category WHERE organization_id = ? AND code = ?)";
const INSERT_CATEGORY: &str = "INSERT INTO category \
     (id, organization_id, code, name, status, version, created_at, updated_at) \
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
const UPDATE_CATEGORY_CAS: &str = "UPDATE category \
     SET name = ?, status = ?, version = ?, updated_at = ? \
     WHERE organization_id = ? AND id = ? AND version = ?";
const SELECT_CATEGORY_VERSION: &str =
    "SELECT version FROM category WHERE organization_id = ? AND id = ?";
const LIST_CATEGORIES: &str = "SELECT id, organization_id, code, name, status, version \
     FROM category WHERE organization_id = ? AND (? IS NULL OR status = ?) \
     ORDER BY name, id LIMIT ? OFFSET ?";

const SELECT_PRODUCT: &str = "SELECT id, organization_id, category_id, sku, barcode, name, \
     description, unit_price, status, version \
     FROM product WHERE organization_id = ? AND id = ?";
const PRODUCT_SKU_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM product WHERE organization_id = ? AND sku = ?)";
const PRODUCT_BARCODE_EXISTS: &str =
    "SELECT EXISTS (SELECT 1 FROM product WHERE organization_id = ? AND barcode = ?)";
const INSERT_PRODUCT: &str = "INSERT INTO product \
     (id, organization_id, category_id, sku, barcode, name, description, unit_price, status, \
      version, created_at, updated_at) \
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
const UPDATE_PRODUCT_CAS: &str = "UPDATE product \
     SET category_id = ?, barcode = ?, name = ?, description = ?, unit_price = ?, status = ?, \
         version = ?, updated_at = ? \
     WHERE organization_id = ? AND id = ? AND version = ?";
const SELECT_PRODUCT_VERSION: &str =
    "SELECT version FROM product WHERE organization_id = ? AND id = ?";
const LIST_PRODUCTS: &str = "SELECT id, organization_id, category_id, sku, barcode, name, \
     description, unit_price, status, version \
     FROM product WHERE organization_id = ? AND (? IS NULL OR status = ?) \
       AND (? IS NULL OR category_id = ?) \
     ORDER BY name, id LIMIT ? OFFSET ?";

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

/// Datos de alta de categoría recibidos por el caso de uso.
///
/// Llegan sin validar: el caso de uso aplica las mismas reglas que Java
/// (`java_trim`, slug en minúsculas, nombre visible).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCategoryRequest {
    /// Código de negocio propuesto.
    pub code: String,
    /// Nombre visible propuesto.
    pub name: String,
}

/// Datos de alta de producto recibidos por el caso de uso.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewProductRequest {
    /// SKU propuesto; se normaliza a mayúsculas canónicas.
    pub sku: String,
    /// Nombre visible propuesto.
    pub name: String,
    /// Precio base en texto decimal, escala máxima 4.
    pub unit_price: String,
    /// Código de barras opcional.
    pub barcode: Option<String>,
    /// Descripción opcional.
    pub description: Option<String>,
    /// Categoría destino de la misma organización.
    pub category_id: Option<EntityId>,
}

/// Cambios de categoría. El código y la organización nunca se modifican.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryUpdateRequest {
    /// Versión leída por el llamador.
    pub expected_version: u64,
    /// Nombre nuevo.
    pub name: Option<String>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Cambios de producto con distinción de ausencia y limpieza.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductUpdateRequest {
    /// Versión leída por el llamador.
    pub expected_version: u64,
    /// Nombre nuevo, ya recortado o no.
    pub name: Option<String>,
    /// Descripción: ausente, limpiada o nueva.
    pub description: PatchField<String>,
    /// Código de barras: ausente, limpiado o nuevo.
    pub barcode: PatchField<String>,
    /// Precio base nuevo.
    pub unit_price: Option<String>,
    /// Categoría: ausente, limpiada o referenciada por identidad.
    pub category_id: PatchField<EntityId>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Ventana de lectura del catálogo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogPage {
    /// Tamaño de página, de 1 a [`MAX_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento desde el inicio del orden por nombre.
    pub offset: u32,
}

/// Tamaño máximo de página admitido por la capa de aplicación.
///
/// El contrato HTTP aún no existe; este límite evita que un transporte futuro
/// solicite el catálogo completo en una sola respuesta.
pub const MAX_PAGE_SIZE: u32 = 200;

/// Casos de uso del catálogo sobre SQLite real.
#[derive(Clone)]
pub struct SqliteCatalogApplication {
    unit_of_work: SqliteUnitOfWork,
}

impl SqliteCatalogApplication {
    /// Ensambla los casos de uso sobre una unidad de trabajo ya configurada.
    pub fn new(unit_of_work: SqliteUnitOfWork) -> Self {
        Self { unit_of_work }
    }

    /// Crea una categoría activa y registra `category.created`.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin permiso,
    /// [`ApplicationError::CategoryCodeAlreadyExists`] si el código está ocupado,
    /// [`ApplicationError::OrganizationNotFound`] si el owner no existe y
    /// [`ApplicationError::InvalidCatalogInput`] si el código o el nombre no
    /// cumplen el contrato.
    pub async fn create_category<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewCategoryRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Category, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::CategoryCreate)?;
        let code = CatalogCode::new(&request.code).map_err(catalog_domain_error)?;
        let name =
            DisplayName::new(&request.name).map_err(|_| ApplicationError::InvalidCatalogInput)?;
        let category_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            if transaction
                .get_organization(&organization_id)
                .await
                .map_err(transaction_error)?
                .is_none()
            {
                return Err(ApplicationError::OrganizationNotFound);
            }
            if transaction
                .category_code_exists(&organization_id, &code)
                .await
                .map_err(catalog_error)?
            {
                return Err(ApplicationError::CategoryCodeAlreadyExists);
            }
            let category = Category::create(NewCategory {
                id: category_id,
                organization_id: organization_id.clone(),
                code,
                name,
            });
            transaction
                .insert_category(&category, occurred_at.as_str())
                .await
                .map_err(catalog_error)?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "category.created",
                "category",
                category.id().clone(),
                json!({ "code": category.code().as_str() }),
                occurred_at,
            )
            .await?;
            Ok(category)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Aplica un patch CAS de categoría y audita solo un cambio material.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin permiso,
    /// [`ApplicationError::CategoryNotFound`],
    /// [`ApplicationError::ConcurrentModification`] o
    /// [`ApplicationError::InvalidCatalogInput`].
    pub async fn update_category<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        category_id: &EntityId,
        request: CategoryUpdateRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Category, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::CategoryUpdate)?;
        let event_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let mut category = transaction
                .get_category(&organization_id, category_id)
                .await
                .map_err(catalog_error)?
                .ok_or(ApplicationError::CategoryNotFound)?;
            let before = category.clone();
            let previous_revision = category.revision();
            let name = match request.name {
                Some(value) => Some(
                    DisplayName::new(&value).map_err(|_| ApplicationError::InvalidCatalogInput)?,
                ),
                None => None,
            };
            if !category
                .patch(CategoryPatch {
                    expected_version: request.expected_version,
                    name,
                    status: request.status,
                })
                .map_err(catalog_domain_error)?
            {
                return Ok(category);
            }
            if transaction
                .update_category_cas(
                    &organization_id,
                    previous_revision,
                    &category,
                    occurred_at.as_str(),
                )
                .await
                .map_err(catalog_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            let changed = category_changed_fields(&before, &category);
            append_audit_with_id(
                &mut transaction,
                event_id,
                principal,
                "category.updated",
                "category",
                category_id.clone(),
                json!({ "changedFields": changed }),
                occurred_at,
            )
            .await?;
            Ok(category)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Crea un producto activo y registra `product.created`.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin permiso,
    /// [`ApplicationError::ProductSkuAlreadyExists`],
    /// [`ApplicationError::ProductBarcodeAlreadyExists`],
    /// [`ApplicationError::CategoryNotFound`],
    /// [`ApplicationError::CategoryInactive`] o
    /// [`ApplicationError::InvalidCatalogInput`].
    pub async fn create_product<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewProductRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Product, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::ProductCreate)?;
        let sku = Sku::new(&request.sku).map_err(catalog_domain_error)?;
        let name =
            DisplayName::new(&request.name).map_err(|_| ApplicationError::InvalidCatalogInput)?;
        let unit_price = UnitPrice::parse(&request.unit_price).map_err(catalog_domain_error)?;
        let barcode = request
            .barcode
            .as_deref()
            .map(Barcode::new)
            .transpose()
            .map_err(catalog_domain_error)?
            .flatten();
        let description = request
            .description
            .as_deref()
            .map(Description::new)
            .transpose()
            .map_err(catalog_domain_error)?
            .flatten();
        let product_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            if transaction
                .get_organization(&organization_id)
                .await
                .map_err(transaction_error)?
                .is_none()
            {
                return Err(ApplicationError::OrganizationNotFound);
            }
            if transaction
                .product_sku_exists(&organization_id, &sku)
                .await
                .map_err(catalog_error)?
            {
                return Err(ApplicationError::ProductSkuAlreadyExists);
            }
            if let Some(existing) = barcode.as_ref()
                && transaction
                    .product_barcode_exists(&organization_id, existing)
                    .await
                    .map_err(catalog_error)?
            {
                return Err(ApplicationError::ProductBarcodeAlreadyExists);
            }
            let category = match request.category_id {
                Some(category_id) => Some(
                    transaction
                        .get_category(&organization_id, &category_id)
                        .await
                        .map_err(catalog_error)?
                        .ok_or(ApplicationError::CategoryNotFound)?,
                ),
                None => None,
            };
            let product = Product::create(NewProduct {
                id: product_id,
                organization_id: organization_id.clone(),
                sku,
                name,
                unit_price,
                barcode,
                description,
                category,
            })
            .map_err(catalog_domain_error)?;
            transaction
                .insert_product(&product, occurred_at.as_str())
                .await
                .map_err(catalog_error)?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "product.created",
                "product",
                product.id().clone(),
                json!({ "sku": product.sku().as_str() }),
                occurred_at,
            )
            .await?;
            Ok(product)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Aplica un patch CAS y audita solo una modificación material.
    ///
    /// Un no-op no escribe auditoría y confirma sin cambios, igual que el
    /// servicio Java.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin permiso,
    /// [`ApplicationError::ProductNotFound`], [`ApplicationError::ConcurrentModification`],
    /// [`ApplicationError::CategoryNotFound`], [`ApplicationError::CategoryInactive`] o
    /// [`ApplicationError::InvalidCatalogInput`].
    pub async fn update_product<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        product_id: &EntityId,
        request: ProductUpdateRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Product, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::ProductUpdate)?;
        let event_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let mut product = transaction
                .get_product(&organization_id, product_id)
                .await
                .map_err(catalog_error)?
                .ok_or(ApplicationError::ProductNotFound)?;
            let before = product.clone();
            let previous_revision = product.revision();
            let category = match &request.category_id {
                PatchField::Absent => PatchField::Absent,
                PatchField::Clear => PatchField::Clear,
                PatchField::Set(category_id) => PatchField::Set(
                    transaction
                        .get_category(&organization_id, category_id)
                        .await
                        .map_err(catalog_error)?
                        .ok_or(ApplicationError::CategoryNotFound)?,
                ),
            };
            let description = match request.description {
                PatchField::Absent => PatchField::Absent,
                PatchField::Clear => PatchField::Clear,
                PatchField::Set(value) => {
                    match Description::new(&value).map_err(catalog_domain_error)? {
                        Some(description) => PatchField::Set(description),
                        None => PatchField::Clear,
                    }
                }
            };
            let barcode = match request.barcode {
                PatchField::Absent => PatchField::Absent,
                PatchField::Clear => PatchField::Clear,
                PatchField::Set(value) => {
                    match Barcode::new(&value).map_err(catalog_domain_error)? {
                        Some(barcode) => PatchField::Set(barcode),
                        None => PatchField::Clear,
                    }
                }
            };
            let name = match request.name {
                Some(value) => Some(
                    DisplayName::new(&value).map_err(|_| ApplicationError::InvalidCatalogInput)?,
                ),
                None => None,
            };
            let unit_price = match request.unit_price {
                Some(value) => Some(UnitPrice::parse(&value).map_err(catalog_domain_error)?),
                None => None,
            };
            let patch = ProductPatch {
                expected_version: request.expected_version,
                name,
                description,
                barcode,
                unit_price,
                category,
                status: request.status,
            };
            if !product.patch(patch).map_err(catalog_domain_error)? {
                return Ok(product);
            }
            if transaction
                .update_product_cas(
                    &organization_id,
                    previous_revision,
                    &product,
                    occurred_at.as_str(),
                )
                .await
                .map_err(catalog_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            let changed = product_changed_fields(&before, &product);
            append_audit_with_id(
                &mut transaction,
                event_id,
                principal,
                "product.updated",
                "product",
                product_id.clone(),
                json!({ "changedFields": changed }),
                occurred_at,
            )
            .await?;
            Ok(product)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Lee una categoría scoped.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] o [`ApplicationError::CategoryNotFound`].
    pub async fn get_category(
        &self,
        principal: &VerifiedPrincipal,
        category_id: &EntityId,
    ) -> Result<Category, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::CategoryRead)?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let category = transaction
            .get_category(&organization_id, category_id)
            .await
            .map_err(catalog_error)?
            .ok_or(ApplicationError::CategoryNotFound);
        finish_read(transaction, category).await
    }

    /// Lee un producto scoped.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] o [`ApplicationError::ProductNotFound`].
    pub async fn get_product(
        &self,
        principal: &VerifiedPrincipal,
        product_id: &EntityId,
    ) -> Result<Product, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::ProductRead)?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let product = transaction
            .get_product(&organization_id, product_id)
            .await
            .map_err(catalog_error)?
            .ok_or(ApplicationError::ProductNotFound);
        finish_read(transaction, product).await
    }

    /// Lista el catálogo de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] o
    /// [`ApplicationError::InvalidCatalogInput`] si la página no es admisible.
    pub async fn list_products(
        &self,
        principal: &VerifiedPrincipal,
        status: Option<Status>,
        category_id: Option<&EntityId>,
        page: CatalogPage,
    ) -> Result<Vec<Product>, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::ProductRead)?;
        if page.limit == 0 || page.limit > MAX_PAGE_SIZE {
            return Err(ApplicationError::InvalidCatalogInput);
        }
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let products = transaction
            .list_products(
                &organization_id,
                status,
                category_id,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await
            .map_err(catalog_error);
        finish_read(transaction, products).await
    }

    /// Lista las categorías de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] o
    /// [`ApplicationError::InvalidCatalogInput`] si la página no es admisible.
    pub async fn list_categories(
        &self,
        principal: &VerifiedPrincipal,
        status: Option<Status>,
        page: CatalogPage,
    ) -> Result<Vec<Category>, ApplicationError> {
        let organization_id = require_catalog_scope(principal, Permission::CategoryRead)?;
        if page.limit == 0 || page.limit > MAX_PAGE_SIZE {
            return Err(ApplicationError::InvalidCatalogInput);
        }
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let categories = transaction
            .list_categories(
                &organization_id,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await
            .map_err(catalog_error);
        finish_read(transaction, categories).await
    }
}

/// Resuelve el scope organizacional y exige el permiso del caso de uso.
fn require_catalog_scope(
    principal: &VerifiedPrincipal,
    permission: Permission,
) -> Result<EntityId, ApplicationError> {
    let code = permission.as_code().ok_or(ApplicationError::Forbidden)?;
    let code = PermissionCode::new(code).map_err(|_| ApplicationError::Forbidden)?;
    // El catálogo pertenece a la organización: se autoriza con el scope
    // organizacional, nunca con el de una sucursal concreta.
    let scope = PrincipalScope::new(principal.scope().organization_id().clone(), None);
    principal
        .authorize(&code, &scope)
        .map_err(|_| ApplicationError::Forbidden)?;
    Ok(scope.organization_id().clone())
}

#[allow(clippy::too_many_arguments)]
async fn append_audit<I: IdentityGenerator>(
    transaction: &mut SqliteTransaction,
    identities: &mut I,
    principal: &VerifiedPrincipal,
    action: &str,
    entity_type: &str,
    entity_id: EntityId,
    metadata: serde_json::Value,
    occurred_at: &AuditOccurredAt,
) -> Result<(), ApplicationError> {
    let event_id = identities.next_id()?;
    append_audit_with_id(
        transaction,
        event_id,
        principal,
        action,
        entity_type,
        entity_id,
        metadata,
        occurred_at,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn append_audit_with_id(
    transaction: &mut SqliteTransaction,
    event_id: EntityId,
    principal: &VerifiedPrincipal,
    action: &str,
    entity_type: &str,
    entity_id: EntityId,
    metadata: serde_json::Value,
    occurred_at: &AuditOccurredAt,
) -> Result<(), ApplicationError> {
    let event = AuditEventRecord::from_verified_principal(
        AuditEventId::new(event_id),
        principal,
        AuditAction::new(action).map_err(|_| ApplicationError::Audit)?,
        AuditEntityType::new(entity_type).map_err(|_| ApplicationError::Audit)?,
        Some(entity_id),
        AuditMetadata::new(metadata).map_err(|_| ApplicationError::Audit)?,
        occurred_at.clone(),
    );
    SqliteAuditWriter::append(transaction, &event).await
}

/// Nombres estables de los campos de categoría modificados (paridad Java).
fn category_changed_fields(before: &Category, after: &Category) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    fields
}

/// Nombres estables de los campos de producto modificados (paridad Java).
fn product_changed_fields(before: &Product, after: &Product) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name");
    }
    if before.description() != after.description() {
        fields.push("description");
    }
    if before.barcode() != after.barcode() {
        fields.push("barcode");
    }
    if before.unit_price() != after.unit_price() {
        fields.push("unitPrice");
    }
    if before.category_id() != after.category_id() {
        fields.push("categoryId");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    fields
}

fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}

fn catalog_error(error: CatalogStorageError) -> ApplicationError {
    match error {
        CatalogStorageError::DuplicateCategoryCode => ApplicationError::CategoryCodeAlreadyExists,
        CatalogStorageError::DuplicateProductSku => ApplicationError::ProductSkuAlreadyExists,
        CatalogStorageError::DuplicateProductBarcode => {
            ApplicationError::ProductBarcodeAlreadyExists
        }
        CatalogStorageError::ForeignKey => ApplicationError::OrganizationNotFound,
        CatalogStorageError::InvalidRow | CatalogStorageError::Database => {
            ApplicationError::Storage
        }
    }
}

fn catalog_domain_error(error: CatalogError) -> ApplicationError {
    match error {
        CatalogError::ConcurrentModification => ApplicationError::ConcurrentModification,
        CatalogError::CategoryInactive => ApplicationError::CategoryInactive,
        CatalogError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        CatalogError::InvalidCode
        | CatalogError::InvalidSku
        | CatalogError::InvalidBarcode
        | CatalogError::InvalidDescription
        | CatalogError::InvalidName
        | CatalogError::InvalidPrice
        | CatalogError::InvalidSelectionRange
        | CatalogError::InvalidDisplayOrder
        | CatalogError::DuplicateModifierGroup
        | CatalogError::RevisionOverflow
        | CatalogError::InvalidPersistedValue => ApplicationError::InvalidCatalogInput,
    }
}

async fn finish<T>(
    transaction: SqliteTransaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError> {
    match operation {
        Ok(value) => {
            transaction.commit().await.map_err(transaction_error)?;
            Ok(value)
        }
        Err(error) => {
            if transaction.rollback().await.is_err() {
                return Err(ApplicationError::Storage);
            }
            Err(error)
        }
    }
}

async fn finish_read<T>(
    transaction: SqliteTransaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError> {
    // Una lectura no necesita reserva de escritura prolongada: se confirma (o
    // revierte) en cuanto se obtiene el resultado y nunca retiene el lock.
    finish(transaction, operation).await
}
