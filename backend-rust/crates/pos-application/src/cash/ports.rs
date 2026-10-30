//! Puertos de aplicación para cajas y turnos.
//!
//! La aplicación no conoce SQLite: declara lo que necesita y el adaptador lo
//! implementa sobre la **misma** transacción física.

use std::future::Future;

use pos_core::{
    audit::AuditOccurredAt,
    cash::{CashRegister, CashShift, CashShiftStatus},
    catalog::CatalogCode,
    identity::UtcSeconds,
    organization_branch::{ApplicationError, Branch, EntityId, Status},
};

use crate::{CasOutcome, audit::AuditAppender};

/// Operaciones transaccionales que necesita el caso de uso de caja.
pub trait CashTransaction: AuditAppender {
    /// Sucursal del scope.
    fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Branch>, ApplicationError>> + Send;
    /// Caja scoped.
    fn get_cash_register(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashRegister>, ApplicationError>> + Send;
    /// Listado determinista de cajas.
    fn list_cash_registers(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<CashRegister>, ApplicationError>> + Send;
    /// Colisión temprana de código de caja.
    fn cash_register_code_exists(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Inserta una caja.
    fn insert_cash_register(
        &mut self,
        register: &CashRegister,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// CAS de caja por revisión.
    fn update_cash_register_cas(
        &mut self,
        previous_revision: u64,
        register: &CashRegister,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Turno abierto de una caja.
    fn get_open_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashShift>, ApplicationError>> + Send;
    /// Turno histórico scoped.
    fn get_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashShift>, ApplicationError>> + Send;
    /// Historial de turnos de una caja.
    fn list_cash_shifts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        status: Option<CashShiftStatus>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<CashShift>, ApplicationError>> + Send;
    /// Precomprobación de turno abierto.
    fn cash_register_has_open_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Convierte el instante auditado al segundo Unix del dominio.
    fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send;
    /// Inserta un turno OPEN.
    fn insert_cash_shift(
        &mut self,
        shift: &CashShift,
        opened_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Cierre condicional por revisión.
    fn close_cash_shift_cas(
        &mut self,
        previous_revision: u64,
        shift: &CashShift,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
}

/// Unidad de trabajo de caja: una transacción física por mutación auditada.
pub trait CashUnitOfWork {
    /// Transacción entregada por el adaptador.
    type Transaction: CashTransaction + Send;
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

/// Puerto de ventas para el guard de cierre de turno.
pub trait OpenSalesForCashShift<T> {
    /// Indica si el turno posee ventas abiertas dentro de esta transacción.
    fn has_open_sales(
        &self,
        transaction: &mut T,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
}
