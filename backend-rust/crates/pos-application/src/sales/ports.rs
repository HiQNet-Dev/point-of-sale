//! Puertos de aplicación para ventas.

use std::future::Future;

use pos_core::{
    audit::AuditOccurredAt,
    catalog::{PriceDelta, Product, UnitPrice},
    identity::UtcSeconds,
    organization_branch::{ApplicationError, EntityId, IdentityGenerator},
    sales::{Sale, SaleItem, SaleScope, SaleStatus},
};

use crate::{
    CasOutcome, audit::AuditAppender, sales::dto::ModifierSnapshot,
    sales_context::SaleOperationalContext,
};

/// Operaciones transaccionales del contexto de ventas.
pub trait SalesTransaction: AuditAppender + SaleOperationalContext {
    /// Venta scoped.
    fn get_sale(
        &mut self,
        scope: &SaleScope,
        sale_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Sale>, ApplicationError>> + Send;
    /// Historial de ventas del turno.
    fn list_sales(
        &mut self,
        scope: &SaleScope,
        status: Option<SaleStatus>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Sale>, ApplicationError>> + Send;
    /// Inserta una venta nueva.
    fn insert_sale(
        &mut self,
        sale: &Sale,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// CAS de venta por revisión.
    fn update_sale_cas(
        &mut self,
        previous: u64,
        sale: &Sale,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Partida concreta de una venta.
    fn get_sale_item(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
        item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<SaleItem>, ApplicationError>> + Send;
    /// Partidas de una venta en orden estable.
    fn list_sale_items(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<SaleItem>, ApplicationError>> + Send;
    /// Siguiente posición libre de partida.
    fn next_sale_item_position(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
    ) -> impl Future<Output = Result<u32, ApplicationError>> + Send;
    /// Inserta una partida.
    fn insert_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Actualiza una partida activa.
    fn update_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Retira lógicamente una partida.
    fn remove_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Opciones seleccionadas de una partida.
    fn item_option_ids(
        &mut self,
        item_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<EntityId>, ApplicationError>> + Send;
    /// Sustituye los snapshots de modificadores de una partida.
    fn replace_item_modifiers<I: IdentityGenerator + Send>(
        &mut self,
        identities: &mut I,
        item: &SaleItem,
        snapshots: &[ModifierSnapshot],
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Cotiza los modificadores seleccionados contra la configuración vigente.
    fn modifier_quote(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
        selected: &[EntityId],
        base: UnitPrice,
    ) -> impl Future<
        Output = Result<(PriceDelta, UnitPrice, Vec<ModifierSnapshot>), ApplicationError>,
    > + Send;
    /// Producto de la organización para cotizar una nueva partida.
    fn get_product(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Product>, ApplicationError>> + Send;
    /// Convierte el instante auditado al segundo Unix del dominio.
    fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send;
}

/// Unidad de trabajo de ventas.
pub trait SalesUnitOfWork {
    /// Transacción entregada por el adaptador.
    type Transaction: SalesTransaction + Send;
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
