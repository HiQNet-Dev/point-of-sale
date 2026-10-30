//! Mappers de fila del contexto.

use super::store::*;
use crate::prelude::*;

#[derive(sqlx::FromRow)]
pub(crate) struct CategoryRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) version: i64,
}

impl CategoryRow {
    pub(crate) fn into_domain(self) -> Result<Category, CatalogStorageError> {
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
pub(crate) struct ProductRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) category_id: Option<String>,
    pub(crate) sku: String,
    pub(crate) barcode: Option<String>,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) unit_price: String,
    pub(crate) status: String,
    pub(crate) version: i64,
}

impl ProductRow {
    pub(crate) fn into_domain(self) -> Result<Product, CatalogStorageError> {
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
