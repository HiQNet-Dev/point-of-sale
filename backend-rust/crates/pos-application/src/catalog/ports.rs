//! Puertos de aplicación del catálogo.

use std::future::Future;

use pos_core::{
    audit::AuditOccurredAt,
    catalog::{Barcode, CatalogCode, Category, Product, Sku},
    identity::UtcSeconds,
    organization_branch::{ApplicationError, EntityId, Status},
};

use crate::{CasOutcome, audit::AuditAppender};

/// Operaciones transaccionales del catálogo organizacional.
pub trait CatalogTransaction: AuditAppender {
    /// Organización existente.
    fn get_organization(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<
        Output = Result<Option<pos_core::organization_branch::Organization>, ApplicationError>,
    > + Send;
    /// Categoría de la organización.
    fn get_category(
        &mut self,
        organization_id: &EntityId,
        category_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Category>, ApplicationError>> + Send;
    /// Colisión de código de categoría.
    fn category_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Inserta una categoría.
    fn insert_category(
        &mut self,
        category: &Category,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// CAS de categoría.
    fn update_category_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        category: &Category,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Listado de categorías.
    fn list_categories(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Category>, ApplicationError>> + Send;
    /// Producto de la organización.
    fn get_product(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Product>, ApplicationError>> + Send;
    /// Colisión de SKU.
    fn product_sku_exists(
        &mut self,
        organization_id: &EntityId,
        sku: &Sku,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Colisión de código de barras.
    fn product_barcode_exists(
        &mut self,
        organization_id: &EntityId,
        barcode: &Barcode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Inserta un producto.
    fn insert_product(
        &mut self,
        product: &Product,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// CAS de producto.
    fn update_product_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        product: &Product,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Listado de productos.
    fn list_products(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        category_id: Option<&EntityId>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Product>, ApplicationError>> + Send;
    /// Convierte el instante auditado al segundo Unix del dominio.
    fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send;
}

/// Unidad de trabajo del catálogo.
pub trait CatalogUnitOfWork {
    /// Transacción entregada por el adaptador.
    type Transaction: CatalogTransaction + Send;
    /// Abre la transacción de escritura.
    fn begin_transaction(
        &self,
    ) -> impl Future<Output = Result<Self::Transaction, ApplicationError>> + Send;
    /// Confirma una sola vez.
    fn commit_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Revierte antes de cualquier confirmación.
    fn rollback_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}
