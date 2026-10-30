//! Puertos de aplicación de inventario.

use std::future::Future;

use pos_core::{
    audit::AuditOccurredAt,
    catalog::CatalogCode,
    identity::UtcSeconds,
    inventory::{InventoryItem, InventoryMovement, StockBalance},
    organization_branch::{ApplicationError, EntityId},
};

use crate::{CasOutcome, audit::AuditAppender};

/// Operaciones transaccionales del inventario por sucursal.
pub trait InventoryTransaction: AuditAppender {
    /// Artículo de la organización.
    fn get_inventory_item(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<InventoryItem>, ApplicationError>> + Send;
    /// Colisión de código de artículo.
    fn inventory_item_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Listado determinista de artículos.
    fn list_inventory_items(
        &mut self,
        organization_id: &EntityId,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<InventoryItem>, ApplicationError>> + Send;
    /// Inserta un artículo.
    fn insert_inventory_item(
        &mut self,
        item: &InventoryItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Congela la unidad base.
    fn lock_inventory_unit(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Saldo de la sucursal.
    fn get_stock_balance(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<StockBalance>, ApplicationError>> + Send;
    /// Inserta el saldo de la primera apertura.
    fn insert_stock_balance(
        &mut self,
        balance: &StockBalance,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// CAS del saldo.
    fn update_stock_balance_cas(
        &mut self,
        previous_revision: u64,
        balance: &StockBalance,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Indica si existe apertura y si existe cualquier movimiento.
    fn ledger_counts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<(bool, bool), ApplicationError>> + Send;
    /// Añade un hecho al ledger append-only.
    fn insert_inventory_movement(
        &mut self,
        movement: &InventoryMovement,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Historial de movimientos.
    fn list_inventory_movements(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<InventoryMovement>, ApplicationError>> + Send;
    /// Convierte el instante auditado al segundo Unix del dominio.
    fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send;
}

/// Unidad de trabajo de inventario.
pub trait InventoryUnitOfWork {
    /// Transacción entregada por el adaptador.
    type Transaction: InventoryTransaction + Send;
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
