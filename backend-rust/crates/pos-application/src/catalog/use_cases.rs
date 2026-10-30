//! Casos de uso del catálogo organizacional.

use pos_core::{
    audit::AuditOccurredAt,
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

use crate::{
    CasOutcome,
    audit::{AuditAppender, append_audit, append_audit_with_id},
    catalog::{dto::*, ports::*},
};

/// Casos de uso del catálogo sobre SQLite real.
#[derive(Clone)]
pub struct CatalogApplication<U> {
    unit_of_work: U,
}

impl<U> CatalogApplication<U>
where
    U: CatalogUnitOfWork,
    U::Transaction: AuditAppender + Send,
{
    /// Ensambla los casos de uso sobre una unidad de trabajo ya configurada.
    pub fn new(unit_of_work: U) -> Self {
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            if transaction
                .get_organization(&organization_id)
                .await?
                .is_none()
            {
                return Err(ApplicationError::OrganizationNotFound);
            }
            if transaction
                .category_code_exists(&organization_id, &code)
                .await?
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
                .await?;
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
        finish(&self.unit_of_work, transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let mut category = transaction
                .get_category(&organization_id, category_id)
                .await?
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
                .await?
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
        finish(&self.unit_of_work, transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            if transaction
                .get_organization(&organization_id)
                .await?
                .is_none()
            {
                return Err(ApplicationError::OrganizationNotFound);
            }
            if transaction
                .product_sku_exists(&organization_id, &sku)
                .await?
            {
                return Err(ApplicationError::ProductSkuAlreadyExists);
            }
            if let Some(existing) = barcode.as_ref()
                && transaction
                    .product_barcode_exists(&organization_id, existing)
                    .await?
            {
                return Err(ApplicationError::ProductBarcodeAlreadyExists);
            }
            let category = match request.category_id {
                Some(category_id) => Some(
                    transaction
                        .get_category(&organization_id, &category_id)
                        .await?
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
                .await?;
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
        finish(&self.unit_of_work, transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let mut product = transaction
                .get_product(&organization_id, product_id)
                .await?
                .ok_or(ApplicationError::ProductNotFound)?;
            let before = product.clone();
            let previous_revision = product.revision();
            let category = match &request.category_id {
                PatchField::Absent => PatchField::Absent,
                PatchField::Clear => PatchField::Clear,
                PatchField::Set(category_id) => PatchField::Set(
                    transaction
                        .get_category(&organization_id, category_id)
                        .await?
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
                .await?
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
        finish(&self.unit_of_work, transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let category = transaction
            .get_category(&organization_id, category_id)
            .await?
            .ok_or(ApplicationError::CategoryNotFound);
        finish_read(&self.unit_of_work, transaction, category).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let product = transaction
            .get_product(&organization_id, product_id)
            .await?
            .ok_or(ApplicationError::ProductNotFound);
        finish_read(&self.unit_of_work, transaction, product).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let products = transaction
            .list_products(
                &organization_id,
                status,
                category_id,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await;
        finish_read(&self.unit_of_work, transaction, products).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let categories = transaction
            .list_categories(
                &organization_id,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await;
        finish_read(&self.unit_of_work, transaction, categories).await
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

async fn finish<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: CatalogUnitOfWork,
{
    match operation {
        Ok(value) => {
            unit_of_work.commit_transaction(transaction).await?;
            Ok(value)
        }
        Err(error) => {
            if unit_of_work
                .rollback_transaction(transaction)
                .await
                .is_err()
            {
                return Err(ApplicationError::Storage);
            }
            Err(error)
        }
    }
}

async fn finish_read<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: CatalogUnitOfWork,
{
    // Una lectura no necesita reserva de escritura prolongada: se confirma (o
    // revierte) en cuanto se obtiene el resultado y nunca retiene el lock.
    unit_of_work.commit_transaction(transaction).await?;
    operation
}
