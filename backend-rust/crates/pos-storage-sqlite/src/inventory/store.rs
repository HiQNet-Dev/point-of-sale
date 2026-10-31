//! Casos de uso candidatos de inventario sobre SQLite real.
//!
//! **No productivo.** Las tablas viven en `proposals/local-inventory/` y las
//! pruebas las aplican de forma explícita; `pos-local::migrate()` no descubre
//! esos archivos. Cada mutación comparte una única UoW `BEGIN IMMEDIATE` con el
//! ledger y la auditoría, y confirma solo si ambos se escribieron.
//!
//! # Invariantes
//!
//! - La apertura es única por sucursal y artículo y precede a cualquier otro
//!   movimiento ([`ensure_opening_allowed`]).
//! - Los ajustes exigen un saldo aperturado y un motivo normalizado.
//! - Las cantidades viajan como `TEXT` decimal canónico escala 6; nunca se usa
//!   `REAL` ni aritmética de punto flotante.
//! - La unidad base se congela en el primer movimiento y no vuelve a liberarse.
//! - El ledger es append-only por trigger; el saldo se materializa por CAS.
//! - La sucursal siempre procede del principal verificado, nunca del cliente.

use pos_core::organization_branch::ApplicationError;

use crate::prelude::transaction_error;
use crate::{SqliteTransaction, SqliteUnitOfWork};

pub(crate) const ITEM_CODE_EXISTS: &str =
    "SELECT EXISTS(SELECT 1 FROM inventory_item WHERE organization_id = ? AND code = ?)";

pub(crate) fn revision_to_i64(value: u64) -> Result<i64, ApplicationError> {
    i64::try_from(value).map_err(|_| storage())
}

pub(crate) fn map_inventory_write(error: &sqlx::Error) -> ApplicationError {
    let Some(database) = error.as_database_error() else {
        return storage();
    };
    if database.is_foreign_key_violation() {
        ApplicationError::OwnershipMismatch
    } else if database.is_check_violation() || database.is_unique_violation() {
        ApplicationError::InvalidInventoryInput
    } else {
        storage()
    }
}

pub use pos_application::inventory::{
    AdjustStockRequest, InventoryApplication, InventoryPage, InventoryTransaction,
    InventoryUnitOfWork, MAX_INVENTORY_PAGE_SIZE, NewInventoryItemRequest, OpenStockRequest,
};

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteInventoryApplication = InventoryApplication<SqliteUnitOfWork>;

impl InventoryUnitOfWork for SqliteUnitOfWork {
    type Transaction = SqliteTransaction;

    async fn begin_transaction(&self) -> Result<SqliteTransaction, ApplicationError> {
        SqliteUnitOfWork::begin(self)
            .await
            .map_err(transaction_error)
    }

    async fn commit_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.commit().await.map_err(transaction_error)
    }

    async fn rollback_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.rollback().await.map_err(transaction_error)
    }
}

pub(crate) fn storage() -> ApplicationError {
    ApplicationError::Storage
}
