//! Compras persistentes sobre el esquema productivo `0016`.
//!
//! Las reglas, la autorización y la auditoría viven en `pos-core` y
//! `pos-application`; este adaptador solo traduce a SQL.
//!
//! # Garantías
//!
//! - Proveedor, orden, partida y recepción comparten la misma `SqliteTransaction`
//!   que la auditoría y el movimiento de stock.
//! - El costo pactado desconocido se persiste como `NULL`, nunca como cero.
//! - La recepción mueve `inventory_balance` y escribe `inventory_ledger` con
//!   `movement_type = 'PURCHASE_RECEIPT'` en la misma confirmación.

mod mapper;
mod queries;
mod repository;

pub use pos_application::purchasing::{
    ChangeSupplierRequest, MAX_PURCHASING_PAGE_SIZE, NewPurchaseOrderLineRequest,
    NewPurchaseOrderRequest, NewSupplierRequest, OptionalField, PostReceiptRequest,
    PurchaseOrderLineView, PurchaseOrderView, PurchaseReceiptLineView, PurchaseReceiptView,
    PurchasingApplication, PurchasingPage, PurchasingTransaction, PurchasingUnitOfWork,
    ReceiptLineInput, ReceiptStatusView, SupplierSummary, UpdatePurchaseOrderLineRequest,
    UpdateSupplierRequest,
};
pub use repository::{PurchasingStorageError, SqlitePurchasingApplication};
