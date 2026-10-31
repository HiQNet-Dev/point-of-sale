//! Casos de uso SQLite candidatos de cajas y turnos.
//!
//! **No productivo todavía.** Las tablas viven en `proposals/local-cash/` y no
//! están cargadas por `pos-local`. Esta composición reutiliza la frontera de
//! principal verificado, `BEGIN IMMEDIATE` y auditoría transaccional para que la
//! promoción aprobada no requiera reescribir reglas de seguridad.
//!
//! El puerto [`OpenSalesForCashShift`] se ejecuta dentro de la misma UoW que el
//! cierre. La futura persistencia de ventas debe usar la misma estrategia de
//! serialización para que crear una venta y cerrar su turno sean una decisión
//! atómica, nunca un `SELECT` rancio seguido de un `UPDATE`.

use pos_core::organization_branch::ApplicationError;

use crate::prelude::transaction_error;
use crate::{SqliteTransaction, SqliteUnitOfWork};

/// Error de persistencia candidato sin exponer SQL ni valores de entrada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CashStorageError {
    /// Código de caja repetido en la misma sucursal.
    DuplicateRegisterCode,
    /// El índice parcial rechazó una segunda apertura concurrente.
    OpenShiftExists,
    /// Una FK de ownership o actor fue rechazada.
    ForeignKey,
    /// La fila incumple un CHECK o no puede rehidratarse.
    InvalidRow,
    /// SQLite rechazó la operación por una causa no clasificable.
    Database,
}

impl std::fmt::Display for CashStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::DuplicateRegisterCode => "código de caja duplicado",
            Self::OpenShiftExists => "la caja ya tiene un turno abierto",
            Self::ForeignKey => "referencia de caja o turno inválida",
            Self::InvalidRow => "fila de caja o turno inválida",
            Self::Database => "operación SQLite de caja o turno fallida",
        })
    }
}
impl std::error::Error for CashStorageError {}

pub(crate) fn revision_to_u64(value: i64) -> Result<u64, CashStorageError> {
    u64::try_from(value).map_err(|_| CashStorageError::InvalidRow)
}

pub(crate) fn revision_to_i64(value: u64) -> Result<i64, CashStorageError> {
    i64::try_from(value).map_err(|_| CashStorageError::InvalidRow)
}

pub(crate) fn map_write_error(error: &sqlx::Error) -> CashStorageError {
    let Some(database) = error.as_database_error() else {
        return CashStorageError::Database;
    };
    if database.is_unique_violation() {
        let message = database.message();
        if message.contains("cash_register.branch_id, cash_register.code") {
            return CashStorageError::DuplicateRegisterCode;
        }
        if message.contains("cash_shift.cash_register_id") {
            return CashStorageError::OpenShiftExists;
        }
    }
    if database.is_foreign_key_violation() {
        return CashStorageError::ForeignKey;
    }
    if database.is_check_violation() {
        return CashStorageError::InvalidRow;
    }
    CashStorageError::Database
}

pub(crate) fn cash_storage_error(error: CashStorageError) -> ApplicationError {
    match error {
        CashStorageError::DuplicateRegisterCode => ApplicationError::CashRegisterCodeAlreadyExists,
        CashStorageError::OpenShiftExists => ApplicationError::CashRegisterAlreadyHasOpenShift,
        CashStorageError::ForeignKey => ApplicationError::OwnershipMismatch,
        CashStorageError::InvalidRow | CashStorageError::Database => ApplicationError::Storage,
    }
}

pub use pos_application::cash::{
    CashApplication, CashPage, CashRegisterUpdateRequest, CashTransaction, CashUnitOfWork,
    CloseCashShiftRequest, MAX_CASH_PAGE_SIZE, NewCashRegisterRequest, OpenCashShiftRequest,
    OpenSalesForCashShift,
};

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteCashApplication<S> = CashApplication<SqliteUnitOfWork, S>;

impl CashUnitOfWork for SqliteUnitOfWork {
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
