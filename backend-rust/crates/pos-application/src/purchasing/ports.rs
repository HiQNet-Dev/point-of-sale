//! Puertos de aplicación para compras.
//!
//! La aplicación no conoce SQLite: declara las operaciones que necesita y el
//! adaptador las implementa sobre **la misma** transacción física, incluida la
//! auditoría y el movimiento de stock de una recepción.

use std::future::Future;

use pos_core::{
    Currency,
    audit::AuditOccurredAt,
    catalog::CatalogCode,
    identity::UtcSeconds,
    inventory::{InventoryItem, Quantity},
    organization_branch::{ApplicationError, EntityId, Status},
    purchasing::{
        PurchaseOrder, PurchaseOrderLine, PurchaseOrderStatus, PurchaseReceipt, PurchaseReceiptLine,
    },
    supplier::Supplier,
};

use crate::{CasOutcome, audit::AuditAppender};

/// Operaciones transaccionales que necesitan los casos de uso de compras.
pub trait PurchasingTransaction: AuditAppender {
    /// Convierte el instante auditado al segundo Unix del dominio.
    fn utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send;

    /// Moneda vigente de la organización, copiada al crear la orden.
    fn organization_currency(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Currency>, ApplicationError>> + Send;

    /// Proveedor de la organización.
    fn get_supplier(
        &mut self,
        organization_id: &EntityId,
        supplier_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Supplier>, ApplicationError>> + Send;

    /// Indica si el slug ya está ocupado en la organización.
    fn supplier_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;

    /// Listado determinista de proveedores.
    fn list_suppliers(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Supplier>, ApplicationError>> + Send;

    /// Inserta un proveedor.
    fn insert_supplier(
        &mut self,
        supplier: &Supplier,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Actualiza un proveedor con CAS por revisión.
    fn update_supplier_cas(
        &mut self,
        previous_revision: u64,
        supplier: &Supplier,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;

    /// Artículo de inventario de la organización.
    fn get_inventory_item(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<InventoryItem>, ApplicationError>> + Send;

    /// Orden con sus partidas dentro de la sucursal del principal.
    #[allow(clippy::type_complexity)]
    fn get_purchase_order(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> impl Future<
        Output = Result<Option<(PurchaseOrder, Vec<PurchaseOrderLine>)>, ApplicationError>,
    > + Send;

    /// Listado determinista de órdenes de la sucursal, con partidas.
    #[allow(clippy::type_complexity)]
    fn list_purchase_orders(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        status: Option<PurchaseOrderStatus>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<(PurchaseOrder, Vec<PurchaseOrderLine>)>, ApplicationError>>
    + Send;

    /// Indica si el código de orden ya está ocupado en la organización.
    fn purchase_order_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;

    /// Inserta una orden con su revisión inicial.
    fn insert_purchase_order(
        &mut self,
        order: &PurchaseOrder,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Actualiza la cabecera con CAS por revisión.
    fn update_purchase_order_cas(
        &mut self,
        previous_revision: u64,
        order: &PurchaseOrder,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;

    /// Inserta una partida.
    fn insert_purchase_order_line(
        &mut self,
        line: &PurchaseOrderLine,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Reemplaza cantidad y costo de una partida existente.
    fn update_purchase_order_line(
        &mut self,
        line: &PurchaseOrderLine,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Retira una partida de un borrador.
    fn delete_purchase_order_line(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
        line_id: &EntityId,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Siguiente número de línea libre de la orden.
    fn next_line_number(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> impl Future<Output = Result<u32, ApplicationError>> + Send;

    /// Cantidades ya recibidas por partida de una orden.
    fn received_quantities(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<(EntityId, Quantity)>, ApplicationError>> + Send;

    /// Recepción previa con la misma clave de idempotencia.
    fn find_receipt_by_idempotency(
        &mut self,
        organization_id: &EntityId,
        idempotency_key: &str,
    ) -> impl Future<Output = Result<Option<PurchaseReceipt>, ApplicationError>> + Send;

    /// Recepción scoped por sucursal.
    fn get_purchase_receipt(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        purchase_receipt_id: &EntityId,
    ) -> impl Future<Output = Result<Option<PurchaseReceipt>, ApplicationError>> + Send;

    /// Recepciones de una sucursal, opcionalmente de una orden.
    fn list_purchase_receipts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        purchase_order_id: Option<&EntityId>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<PurchaseReceipt>, ApplicationError>> + Send;

    /// Líneas de una recepción.
    fn receipt_lines(
        &mut self,
        organization_id: &EntityId,
        purchase_receipt_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<PurchaseReceiptLine>, ApplicationError>> + Send;

    /// Inserta la cabecera de la recepción.
    fn insert_purchase_receipt(
        &mut self,
        receipt: &PurchaseReceipt,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Inserta una línea de recepción.
    fn insert_purchase_receipt_line(
        &mut self,
        line: &PurchaseReceiptLine,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Aplica una entrada de stock y su movimiento de ledger.
    #[allow(clippy::too_many_arguments)]
    fn apply_purchase_receipt_stock(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
        quantity: Quantity,
        receipt_line_id: &EntityId,
        ledger_id: &EntityId,
        balance_id: &EntityId,
        actor_user_id: &EntityId,
        occurred_at_seconds: i64,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Cantidad en existencia de un artículo en la sucursal.
    fn inventory_balance(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Quantity>, ApplicationError>> + Send;
}

/// Unidad de trabajo de compras: una transacción física por mutación auditada.
pub trait PurchasingUnitOfWork {
    /// Transacción entregada por el adaptador.
    type Transaction: PurchasingTransaction + Send;
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
