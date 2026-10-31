//! Compras persistentes sobre el esquema candidato `0016`.
//!
//! **No productivo todavía.** Las tablas viven en
//! `proposals/local-purchasing/` y `pos-local` no las carga. El adaptador existe
//! para que la activación aprobada solo requiera promover la migración y conectar
//! la composición: las reglas, la autorización y la auditoría ya viven en
//! `pos-core` y `pos-application`.
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
