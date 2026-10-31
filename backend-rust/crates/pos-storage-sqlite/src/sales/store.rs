//! Casos de uso de ventas sobre SQLite real.
//!
//! Las tablas las crea el migrador aprobado (`0011`, `0012`). Cada mutación usa
//! una sola UoW `BEGIN IMMEDIATE`, conserva snapshots históricos y escribe
//! auditoría antes del único commit. Checkout vive en su propio módulo porque
//! añade pagos, consumo de inventario y cierre atómico.

use std::future::Future;

use pos_core::organization_branch::{ApplicationError, EntityId};

use crate::prelude::transaction_error;
use crate::{SqliteTransaction, SqliteUnitOfWork, cash::OpenSalesForCashShift};

pub(crate) const OPEN_SALE_EXISTS: &str =
    "SELECT EXISTS(SELECT 1 FROM sale WHERE cash_shift_id = ? AND status = 'OPEN')";

/// Error de infraestructura de ventas candidato sin exponer SQL ni entradas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SalesStorageError {
    /// FK de scope, producto, turno o actor rechazada por SQLite.
    ForeignKey,
    /// CHECK, unicidad de posición o fila incoherente.
    InvalidRow,
    /// Error SQLite no clasificable.
    Database,
}

impl std::fmt::Display for SalesStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ForeignKey => "referencia de venta inválida",
            Self::InvalidRow => "fila de venta inválida",
            Self::Database => "operación SQLite de venta fallida",
        })
    }
}
impl std::error::Error for SalesStorageError {}

pub(crate) fn map_write_error(error: &sqlx::Error) -> SalesStorageError {
    let Some(database) = error.as_database_error() else {
        return SalesStorageError::Database;
    };
    if database.is_foreign_key_violation() {
        SalesStorageError::ForeignKey
    } else if database.is_check_violation() || database.is_unique_violation() {
        SalesStorageError::InvalidRow
    } else {
        SalesStorageError::Database
    }
}

pub(crate) fn revision_to_i64(value: u64) -> Result<i64, SalesStorageError> {
    i64::try_from(value).map_err(|_| SalesStorageError::InvalidRow)
}
pub(crate) fn revision_to_u64(value: i64) -> Result<u64, SalesStorageError> {
    u64::try_from(value).map_err(|_| SalesStorageError::InvalidRow)
}

/// Adaptador concreto del puerto de cajas; consulta `sale` dentro de la misma UoW.
#[derive(Clone, Copy, Debug, Default)]
pub struct SqliteOpenSalesForCashShift;

impl OpenSalesForCashShift<SqliteTransaction> for SqliteOpenSalesForCashShift {
    fn has_open_sales(
        &self,
        transaction: &mut SqliteTransaction,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let cash_shift_id = cash_shift_id.clone();
        async move {
            let exists: i64 = sqlx::query_scalar(OPEN_SALE_EXISTS)
                .bind(cash_shift_id.as_str())
                .fetch_one(transaction.connection())
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Ok(exists != 0)
        }
    }
}

pub(crate) fn storage_error(error: SalesStorageError) -> ApplicationError {
    match error {
        SalesStorageError::ForeignKey => ApplicationError::OwnershipMismatch,
        _ => ApplicationError::Storage,
    }
}

pub use pos_application::sales::{
    AddSaleItemRequest, MAX_SALE_PAGE_SIZE, ModifierSnapshot, NewSaleRequest, SalePage,
    UpdateSaleItemRequest,
};

pub use pos_application::sales::{SalesApplication, SalesTransaction, SalesUnitOfWork, sale_error};

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteSalesApplication = SalesApplication<SqliteUnitOfWork>;

impl SalesUnitOfWork for SqliteUnitOfWork {
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
