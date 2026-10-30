//! Contexto operativo de una venta dentro de su unidad de trabajo.
//!
//! El caso de uso de ventas exige que sucursal, caja y turno existan, estén
//! activos y pertenezcan al scope antes de abrir, cotizar o cobrar. Esa lectura
//! debe ocurrir en la **misma transacción** que la mutación, por eso el puerto
//! expone lecturas sobre el mismo objeto transaccional que el adaptador de
//! persistencia entrega.
//!
//! # Invariantes
//!
//! - La sucursal debe estar `ACTIVE`.
//! - La caja debe estar `ACTIVE`.
//! - El turno debe estar `OPEN`.
//! - Toda lectura está filtrada por organización, sucursal, caja y turno.

use std::future::Future;

use pos_core::{
    cash::{CashRegister, CashShift, CashShiftStatus},
    organization_branch::{ApplicationError, Branch, EntityId, Status},
    sales::SaleScope,
};

/// Lecturas transaccionales necesarias para validar el contexto de una venta.
pub trait SaleOperationalContext {
    /// Sucursal de la organización indicada.
    fn find_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Branch>, ApplicationError>> + Send;
    /// Caja de la sucursal indicada.
    fn find_cash_register(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashRegister>, ApplicationError>> + Send;
    /// Turno de la caja indicada.
    fn find_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashShift>, ApplicationError>> + Send;
}

/// Exige sucursal activa, caja activa y turno abierto en el scope solicitado.
///
/// # Errors
/// Devuelve [`ApplicationError::BranchNotFound`], `BranchInactive`,
/// `CashRegisterNotFound`, `CashRegisterInactive`, `CashShiftNotFound` o
/// `CashShiftAlreadyClosed` en el mismo orden que el servicio Java.
pub async fn require_open_operational_shift<T>(
    context: &mut T,
    scope: &SaleScope,
) -> Result<(), ApplicationError>
where
    T: SaleOperationalContext + Send,
{
    let branch = context
        .find_branch(&scope.organization_id, &scope.branch_id)
        .await?
        .ok_or(ApplicationError::BranchNotFound)?;
    if branch.status() != Status::Active {
        return Err(ApplicationError::BranchInactive);
    }
    let register = context
        .find_cash_register(
            &scope.organization_id,
            &scope.branch_id,
            &scope.cash_register_id,
        )
        .await?
        .ok_or(ApplicationError::CashRegisterNotFound)?;
    if !register.is_active() {
        return Err(ApplicationError::CashRegisterInactive);
    }
    let shift = context
        .find_cash_shift(
            &scope.organization_id,
            &scope.branch_id,
            &scope.cash_register_id,
            &scope.cash_shift_id,
        )
        .await?
        .ok_or(ApplicationError::CashShiftNotFound)?;
    if shift.status() != CashShiftStatus::Open {
        return Err(ApplicationError::CashShiftAlreadyClosed);
    }
    Ok(())
}
