//! Catálogo sobre la sesión autenticada.
//!
//! Cada comando revalida el principal en SQLite y delega en
//! `pos-application::catalog`. El cliente no aporta organización, actor ni
//! permisos: solo datos de negocio y la versión que leyó.
//!
//! Los precios viajan como texto decimal exacto de escala 4; nunca se convierten
//! a coma flotante.

use pos_application::catalog::{
    CatalogApplication, CatalogPage, NewCategoryRequest, NewProductRequest, ProductUpdateRequest,
};
use pos_core::{
    catalog::{Category, PatchField, Product},
    organization_branch::{EntityId, Status},
};
use pos_storage_sqlite::SqliteUnitOfWork;

use crate::{
    auth::SecureIdentities,
    composition::LocalState,
    dto::{
        CatalogItemRequestDto, CategoryDto, CreateProductRequest, CreatedProductDto, IpcError,
        NewCategoryRequestDto, PageRequest, ProductDto, UpdateProductRequestDto,
    },
};

impl LocalState {
    /// Lista categorías de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] sin sesión y [`IpcError::Forbidden`]
    /// sin `category.read`.
    pub async fn catalog_list_categories(
        &self,
        page: PageRequest,
    ) -> Result<Vec<CategoryDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let application = CatalogApplication::new(SqliteUnitOfWork::new(
            self.product_installation()?.pool.clone(),
        ));
        let categories = application
            .list_categories(
                &principal,
                None,
                CatalogPage {
                    limit: page.limit,
                    offset: page.offset,
                },
            )
            .await?;
        Ok(categories.iter().map(CategoryDto::from).collect())
    }

    /// Lista productos de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] sin sesión y [`IpcError::Forbidden`]
    /// sin `product.read`.
    pub async fn catalog_list_products(
        &self,
        page: PageRequest,
    ) -> Result<Vec<ProductDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let application = CatalogApplication::new(SqliteUnitOfWork::new(
            self.product_installation()?.pool.clone(),
        ));
        let products = application
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
        Ok(products.iter().map(ProductDto::from).collect())
    }

    /// Consulta un producto de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`IpcError::NotFound`] si no pertenece al tenant.
    pub async fn catalog_get_product(
        &self,
        request: CatalogItemRequestDto,
    ) -> Result<ProductDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let product_id = EntityId::parse(&request.id).map_err(|_| IpcError::ValidationError)?;
        let application = CatalogApplication::new(SqliteUnitOfWork::new(
            self.product_installation()?.pool.clone(),
        ));
        let product = application.get_product(&principal, &product_id).await?;
        Ok(ProductDto::from(&product))
    }

    /// Crea una categoría y devuelve su versión inicial.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `category.create` y
    /// [`IpcError::Conflict`] si el código ya existe.
    pub async fn catalog_create_category(
        &self,
        request: NewCategoryRequestDto,
    ) -> Result<CategoryDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let application = CatalogApplication::new(SqliteUnitOfWork::new(
            self.product_installation()?.pool.clone(),
        ));
        let mut identities = SecureIdentities;
        let category = application
            .create_category(
                &principal,
                &mut identities,
                NewCategoryRequest {
                    code: request.code,
                    name: request.name,
                },
                &occurred_at,
            )
            .await?;
        Ok(CategoryDto::from(&category))
    }

    /// Crea un producto y devuelve su versión inicial.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `product.create` y
    /// [`IpcError::Conflict`] si el SKU o el código de barras ya existen.
    pub async fn catalog_create_product(
        &self,
        request: CreateProductRequest,
    ) -> Result<CreatedProductDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let application = CatalogApplication::new(SqliteUnitOfWork::new(
            self.product_installation()?.pool.clone(),
        ));
        let mut identities = SecureIdentities;
        let product = application
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

    /// Edita un producto con CAS sobre la versión leída.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `product.update`,
    /// [`IpcError::NotFound`] si no pertenece al tenant y
    /// [`IpcError::Conflict`] si la versión cambió.
    pub async fn catalog_update_product(
        &self,
        request: UpdateProductRequestDto,
    ) -> Result<ProductDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let product_id =
            EntityId::parse(&request.product_id).map_err(|_| IpcError::ValidationError)?;
        let expected_version = request
            .expected_version
            .parse::<u64>()
            .map_err(|_| IpcError::ValidationError)?;
        let status = match request.status.as_deref() {
            Some("ACTIVE") => Some(Status::Active),
            Some("INACTIVE") => Some(Status::Inactive),
            Some(_) => return Err(IpcError::ValidationError),
            None => None,
        };
        let category_id = match request.category_id {
            Some(value) => {
                PatchField::Set(EntityId::parse(&value).map_err(|_| IpcError::ValidationError)?)
            }
            None => PatchField::Absent,
        };
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let application = CatalogApplication::new(SqliteUnitOfWork::new(
            self.product_installation()?.pool.clone(),
        ));
        let mut identities = SecureIdentities;
        let product = application
            .update_product(
                &principal,
                &mut identities,
                &product_id,
                ProductUpdateRequest {
                    expected_version,
                    name: request.name,
                    description: PatchField::Absent,
                    barcode: PatchField::Absent,
                    unit_price: request.unit_price,
                    category_id,
                    status,
                },
                &occurred_at,
            )
            .await?;
        Ok(ProductDto::from(&product))
    }
}

impl From<&Category> for CategoryDto {
    fn from(category: &Category) -> Self {
        Self {
            id: category.id().as_str().into(),
            code: category.code().as_str().into(),
            name: category.name().as_str().into(),
            version: category.revision().to_string(),
        }
    }
}

impl From<&Product> for ProductDto {
    fn from(product: &Product) -> Self {
        Self {
            id: product.id().as_str().into(),
            sku: product.sku().as_str().into(),
            name: product.name().as_str().into(),
            unit_price: product.unit_price().to_string(),
            category_id: product.category_id().map(|id| id.as_str().into()),
            version: product.revision().to_string(),
        }
    }
}
