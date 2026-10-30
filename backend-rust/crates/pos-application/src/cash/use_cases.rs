//! Casos de uso de cajas y turnos sobre puertos de aplicación.

use pos_core::{
    audit::AuditOccurredAt,
    cash::{
        CashError, CashRegister, CashRegisterPatch, CashShift, CashShiftStatus, CloseCashShift,
        NewCashRegister, NewCashShift, cash_register_changed_fields, ensure_close_allowed,
        ensure_open_allowed,
    },
    catalog::{CatalogCode, UnitPrice},
    identity::{ScopeMode, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, DisplayName, EntityId, IdentityGenerator, Permission, Status,
    },
};
use serde_json::json;

use crate::{
    CasOutcome,
    audit::{AuditAppender, append_audit, append_audit_with_id},
    authorization::require_branch_scope,
    cash::{dto::*, ports::*},
};

/// Casos de uso candidatos de cajas y turnos sobre SQLite real.
pub struct CashApplication<U, S> {
    unit_of_work: U,
    open_sales: S,
}

impl<U, S> CashApplication<U, S>
where
    U: CashUnitOfWork,
    U::Transaction: AuditAppender + Send,
    S: OpenSalesForCashShift<U::Transaction>,
{
    /// Ensambla los casos de uso con un guard de ventas explícito.
    pub fn new(unit_of_work: U, open_sales: S) -> Self {
        Self {
            unit_of_work,
            open_sales,
        }
    }

    /// Crea una caja activa y audita `cash-register.created` dentro del commit.
    pub async fn create_cash_register<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewCashRegisterRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashRegister, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterCreate, false)?;
        let code =
            CatalogCode::new(&request.code).map_err(|_| ApplicationError::InvalidCashInput)?;
        let name =
            DisplayName::new(&request.name).map_err(|_| ApplicationError::InvalidCashInput)?;
        let register_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let branch = transaction
                .get_branch(&organization_id, &branch_id)
                .await?
                .ok_or(ApplicationError::BranchNotFound)?;
            if transaction
                .cash_register_code_exists(&organization_id, &branch_id, &code)
                .await?
            {
                return Err(ApplicationError::CashRegisterCodeAlreadyExists);
            }
            let register = CashRegister::create(NewCashRegister {
                id: register_id,
                organization_id: organization_id.clone(),
                branch_id: branch.id().clone(),
                code,
                name,
            });
            transaction
                .insert_cash_register(&register, occurred_at.as_str())
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "cash-register.created",
                "cash_register",
                register.id().clone(),
                json!({"branchId": branch_id.as_str(), "code": register.code().as_str()}),
                occurred_at,
            )
            .await?;
            Ok(register)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Lee una caja scoped por organización y sucursal.
    pub async fn get_cash_register(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
    ) -> Result<CashRegister, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterRead, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let result = transaction
            .get_cash_register(&organization_id, &branch_id, cash_register_id)
            .await?
            .ok_or(ApplicationError::CashRegisterNotFound);
        finish(&self.unit_of_work, transaction, result).await
    }

    /// Lista cajas de la sucursal del principal con límites explícitos.
    pub async fn list_cash_registers(
        &self,
        principal: &VerifiedPrincipal,
        status: Option<Status>,
        page: CashPage,
    ) -> Result<Vec<CashRegister>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterRead, false)?;
        if page.limit == 0 || page.limit > MAX_CASH_PAGE_SIZE {
            return Err(ApplicationError::InvalidCashInput);
        }
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let result = transaction
            .list_cash_registers(
                &organization_id,
                &branch_id,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await;
        finish(&self.unit_of_work, transaction, result).await
    }

    /// Actualiza nombre o estado por CAS, sin auditar no-ops.
    pub async fn update_cash_register<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: &EntityId,
        request: CashRegisterUpdateRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashRegister, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterUpdate, false)?;
        let event_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let mut register = transaction
                .get_cash_register(&organization_id, &branch_id, cash_register_id)
                .await?
                .ok_or(ApplicationError::CashRegisterNotFound)?;
            let before = register.clone();
            let previous_revision = register.revision();
            let name = request
                .name
                .as_deref()
                .map(DisplayName::new)
                .transpose()
                .map_err(|_| ApplicationError::InvalidCashInput)?;
            if !register
                .patch(CashRegisterPatch {
                    expected_version: request.expected_version,
                    name,
                    status: request.status,
                })
                .map_err(cash_domain_error)?
            {
                return Ok(register);
            }
            if transaction
                .update_cash_register_cas(previous_revision, &register, occurred_at.as_str())
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            append_audit_with_id(
                &mut transaction,
                event_id,
                principal,
                "cash-register.updated",
                "cash_register",
                register.id().clone(),
                json!({"changedFields": cash_register_changed_fields(&before, &register)}),
                occurred_at,
            )
            .await?;
            Ok(register)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Abre un turno tras comprobar branch/caja activas; el índice parcial decide carreras.
    pub async fn open_cash_shift<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: OpenCashShiftRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashShift, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftOpen, false)?;
        let opening_amount = UnitPrice::parse(&request.opening_amount)
            .map_err(|_| ApplicationError::InvalidCashInput)?;
        let shift_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let branch = transaction
                .get_branch(&organization_id, &branch_id)
                .await?
                .ok_or(ApplicationError::BranchNotFound)?;
            let register = transaction
                .get_cash_register(&organization_id, &branch_id, &request.cash_register_id)
                .await?
                .ok_or(ApplicationError::CashRegisterNotFound)?;
            let has_open_shift = transaction
                .cash_register_has_open_shift(
                    &organization_id,
                    &branch_id,
                    &request.cash_register_id,
                )
                .await?;
            ensure_open_allowed(branch.status() == Status::Active, &register, has_open_shift)
                .map_err(cash_domain_error)?;
            let opened_at = transaction.candidate_utc_seconds(occurred_at).await?;
            let shift = CashShift::open(NewCashShift {
                id: shift_id,
                register,
                opened_by_user_id: principal.user_id().clone(),
                opening_amount,
                opened_at,
            })
            .map_err(cash_domain_error)?;
            transaction
                .insert_cash_shift(&shift, occurred_at.as_str())
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "cash-shift.opened",
                "cash_shift",
                shift.id().clone(),
                json!({
                    "branchId": branch_id.as_str(),
                    "cashRegisterId": request.cash_register_id.as_str(),
                    "openingAmount": shift.opening_amount().to_string()
                }),
                occurred_at,
            )
            .await?;
            Ok(shift)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Lee el turno abierto de una caja scoped.
    pub async fn get_open_cash_shift(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
    ) -> Result<Option<CashShift>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftRead, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let result = transaction
            .get_open_cash_shift(&organization_id, &branch_id, cash_register_id)
            .await;
        finish(&self.unit_of_work, transaction, result).await
    }

    /// Lee un turno histórico scoped.
    pub async fn get_cash_shift(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
    ) -> Result<CashShift, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftRead, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let result = transaction
            .get_cash_shift(
                &organization_id,
                &branch_id,
                cash_register_id,
                cash_shift_id,
            )
            .await?
            .ok_or(ApplicationError::CashShiftNotFound);
        finish(&self.unit_of_work, transaction, result).await
    }

    /// Lista el historial de turnos scoped.
    pub async fn list_cash_shifts(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
        status: Option<CashShiftStatus>,
        page: CashPage,
    ) -> Result<Vec<CashShift>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftRead, false)?;
        if page.limit == 0 || page.limit > MAX_CASH_PAGE_SIZE {
            return Err(ApplicationError::InvalidCashInput);
        }
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let result = transaction
            .list_cash_shifts(
                &organization_id,
                &branch_id,
                cash_register_id,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await;
        finish(&self.unit_of_work, transaction, result).await
    }

    /// Cierra un turno OPEN con CAS y guard de ventas dentro de la misma UoW.
    ///
    /// El scope de recuperación es admisible solo aquí: es obligatorio si la
    /// sucursal o caja ya está inactiva y permite finalizar el turno histórico,
    /// pero no abrir ni administrar recursos.
    pub async fn close_cash_shift<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
        request: CloseCashShiftRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashShift, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftClose, true)?;
        let declared_amount = UnitPrice::parse(&request.closing_declared_amount)
            .map_err(|_| ApplicationError::InvalidCashInput)?;
        let event_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            // El cierre normal opera sobre recursos activos. Si caja o sucursal
            // se desactivaron después de abrir el turno, solo una capacidad de
            // recuperación explícita puede finalizar el hecho histórico.
            let branch = transaction
                .get_branch(&organization_id, &branch_id)
                .await?
                .ok_or(ApplicationError::BranchNotFound)?;
            let register = transaction
                .get_cash_register(&organization_id, &branch_id, cash_register_id)
                .await?
                .ok_or(ApplicationError::CashRegisterNotFound)?;
            if (branch.status() != Status::Active || !register.is_active())
                && principal.scope().mode() != ScopeMode::Recovery
            {
                return Err(ApplicationError::Forbidden);
            }
            let mut shift = transaction
                .get_cash_shift(
                    &organization_id,
                    &branch_id,
                    cash_register_id,
                    cash_shift_id,
                )
                .await?
                .ok_or(ApplicationError::CashShiftNotFound)?;
            let has_open_sales = self
                .open_sales
                .has_open_sales(&mut transaction, cash_shift_id)
                .await?;
            ensure_close_allowed(&shift, has_open_sales).map_err(cash_domain_error)?;
            let previous_revision = shift.revision();
            let closed_at = transaction.candidate_utc_seconds(occurred_at).await?;
            shift
                .close(CloseCashShift {
                    expected_version: request.expected_version,
                    declared_amount,
                    closed_by_user_id: principal.user_id().clone(),
                    closed_at,
                })
                .map_err(cash_domain_error)?;
            if transaction
                .close_cash_shift_cas(previous_revision, &shift, occurred_at.as_str())
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            append_audit_with_id(
                &mut transaction,
                event_id,
                principal,
                "cash-shift.closed",
                "cash_shift",
                shift.id().clone(),
                json!({
                    "cashRegisterId": cash_register_id.as_str(),
                    "declaredCashAmount": declared_amount.to_string()
                }),
                occurred_at,
            )
            .await?;
            Ok(shift)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }
}

fn cash_domain_error(error: CashError) -> ApplicationError {
    match error {
        CashError::RegisterInactive => ApplicationError::CashRegisterInactive,
        CashError::BranchInactive => ApplicationError::BranchInactive,
        CashError::RegisterAlreadyHasOpenShift => ApplicationError::CashRegisterAlreadyHasOpenShift,
        CashError::ShiftAlreadyClosed => ApplicationError::CashShiftAlreadyClosed,
        CashError::ShiftHasOpenSales => ApplicationError::CashShiftHasOpenSales,
        CashError::ConcurrentModification => ApplicationError::ConcurrentModification,
        CashError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        CashError::InvalidAmount
        | CashError::RevisionOverflow
        | CashError::InvalidPersistedValue => ApplicationError::InvalidCashInput,
    }
}

/// Confirma o revierte a través de la unidad de trabajo, nunca de la conexión.
async fn finish<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: CashUnitOfWork,
{
    match operation {
        Ok(value) => {
            unit_of_work.commit_transaction(transaction).await?;
            Ok(value)
        }
        Err(error) => {
            unit_of_work.rollback_transaction(transaction).await?;
            Err(error)
        }
    }
}
