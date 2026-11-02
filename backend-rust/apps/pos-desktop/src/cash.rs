//! Cajas y turnos sobre la sesión autenticada.
//!
//! Cada comando revalida el principal en SQLite y delega en
//! `pos-application::cash`. El cliente no aporta organización, sucursal, actor ni
//! permisos: la sucursal sale del scope verificado y el permiso se resuelve contra
//! el scope exacto que ya calculó SQLite.
//!
//! # Recuperación
//!
//! El cierre admite el scope de recuperación del contrato: puede finalizar un
//! turno histórico sobre una caja o sucursal ya inactiva. La apertura y la
//! administración de cajas **no** lo admiten, porque crear recursos nuevos en un
//! recurso desactivado no es una operación de recuperación.
//!
//! # Importes
//!
//! Los importes viajan como texto decimal exacto de escala máxima 4 y se parsean
//! con `UnitPrice`; nunca se convierten a coma flotante.

use pos_application::cash::{
    CashPage, CashRegisterUpdateRequest, CloseCashShiftRequest, NewCashRegisterRequest,
    OpenCashShiftRequest,
};
use pos_core::{
    cash::{CashRegister, CashShift, CashShiftStatus},
    organization_branch::{EntityId, Permission, Status},
};
use pos_storage_sqlite::{
    SqliteUnitOfWork, cash::SqliteCashApplication, sales::SqliteOpenSalesForCashShift,
};

use crate::{
    auth::SecureIdentities,
    composition::LocalState,
    dto::{
        CashRegisterDto, CashRegisterListRequestDto, CashShiftDto, CashShiftItemRequestDto,
        CashShiftListRequestDto, CloseCashShiftRequestDto, IpcError, NewCashRegisterRequestDto,
        OpenCashShiftRequestDto, UpdateCashRegisterRequestDto,
    },
};

impl LocalState {
    /// Lista las cajas de la sucursal seleccionada.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] sin sesión, [`IpcError::Forbidden`]
    /// sin sucursal seleccionada o sin `cash-register.read`, y
    /// [`IpcError::ValidationError`] con una página inválida.
    pub async fn cash_register_list(
        &self,
        request: CashRegisterListRequestDto,
    ) -> Result<Vec<CashRegisterDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let status = request.status.as_deref().map(parse_status).transpose()?;
        let registers = self
            .cash()?
            .list_cash_registers(
                &principal,
                status,
                CashPage {
                    limit: request.limit,
                    offset: request.offset,
                },
            )
            .await?;
        Ok(registers.iter().map(CashRegisterDto::from).collect())
    }

    /// Lee una caja de la sucursal seleccionada.
    ///
    /// # Errors
    /// Devuelve [`IpcError::NotFound`] si no pertenece a la sucursal y
    /// [`IpcError::Forbidden`] sin `cash-register.read`.
    pub async fn cash_register_get(
        &self,
        cash_register_id: &str,
    ) -> Result<CashRegisterDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let register_id = parse_id(cash_register_id)?;
        let register = self
            .cash()?
            .get_cash_register(&principal, &register_id)
            .await?;
        Ok(CashRegisterDto::from(&register))
    }

    /// Crea una caja activa en la sucursal seleccionada.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `cash-register.create` y
    /// [`IpcError::Conflict`] si el código ya existe en la sucursal.
    pub async fn cash_register_create(
        &self,
        request: NewCashRegisterRequestDto,
    ) -> Result<CashRegisterDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let register = self
            .cash()?
            .create_cash_register(
                &principal,
                &mut identities,
                NewCashRegisterRequest {
                    code: request.code,
                    name: request.name,
                },
                &occurred_at,
            )
            .await?;
        Ok(CashRegisterDto::from(&register))
    }

    /// Actualiza nombre o estado de una caja con CAS.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `cash-register.update`,
    /// [`IpcError::NotFound`] si no pertenece a la sucursal y
    /// [`IpcError::Conflict`] con una revisión obsoleta.
    pub async fn cash_register_update(
        &self,
        request: UpdateCashRegisterRequestDto,
    ) -> Result<CashRegisterDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let register_id = parse_id(&request.cash_register_id)?;
        let status = request.status.as_deref().map(parse_status).transpose()?;
        let expected_version = parse_revision(&request.expected_revision)?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let register = self
            .cash()?
            .update_cash_register(
                &principal,
                &mut identities,
                &register_id,
                CashRegisterUpdateRequest {
                    expected_version,
                    name: request.name,
                    status,
                },
                &occurred_at,
            )
            .await?;
        Ok(CashRegisterDto::from(&register))
    }

    /// Abre un turno en una caja activa de la sucursal seleccionada.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `cash-shift.open`,
    /// [`IpcError::Conflict`] si la caja ya tiene un turno abierto o está
    /// inactiva y [`IpcError::ValidationError`] con un importe inválido.
    pub async fn cash_shift_open(
        &self,
        request: OpenCashShiftRequestDto,
    ) -> Result<CashShiftDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let register_id = parse_id(&request.cash_register_id)?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let shift = self
            .cash()?
            .open_cash_shift(
                &principal,
                &mut identities,
                OpenCashShiftRequest {
                    cash_register_id: register_id,
                    opening_amount: request.opening_amount,
                },
                &occurred_at,
            )
            .await?;
        Ok(CashShiftDto::from(&shift))
    }

    /// Devuelve el turno abierto de una caja, si existe.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `cash-shift.read`.
    pub async fn cash_shift_get_open(
        &self,
        cash_register_id: &str,
    ) -> Result<Option<CashShiftDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let register_id = parse_id(cash_register_id)?;
        let shift = self
            .cash()?
            .get_open_cash_shift(&principal, &register_id)
            .await?;
        Ok(shift.as_ref().map(CashShiftDto::from))
    }

    /// Lee un turno concreto de una caja.
    ///
    /// # Errors
    /// Devuelve [`IpcError::NotFound`] si no pertenece a la sucursal y
    /// [`IpcError::Forbidden`] sin `cash-shift.read`.
    pub async fn cash_shift_get(
        &self,
        request: CashShiftItemRequestDto,
    ) -> Result<CashShiftDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let register_id = parse_id(&request.cash_register_id)?;
        let shift_id = parse_id(&request.cash_shift_id)?;
        let shift = self
            .cash()?
            .get_cash_shift(&principal, &register_id, &shift_id)
            .await?;
        Ok(CashShiftDto::from(&shift))
    }

    /// Lista el historial de turnos de una caja.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `cash-shift.read` y
    /// [`IpcError::ValidationError`] con una página o un estado inválidos.
    pub async fn cash_shift_list(
        &self,
        request: CashShiftListRequestDto,
    ) -> Result<Vec<CashShiftDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let register_id = parse_id(&request.cash_register_id)?;
        let status = request
            .status
            .as_deref()
            .map(parse_shift_status)
            .transpose()?;
        let shifts = self
            .cash()?
            .list_cash_shifts(
                &principal,
                &register_id,
                status,
                CashPage {
                    limit: request.limit,
                    offset: request.offset,
                },
            )
            .await?;
        Ok(shifts.iter().map(CashShiftDto::from).collect())
    }

    /// Cierra un turno abierto con CAS y guard de ventas `OPEN`.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `cash-shift.close`,
    /// [`IpcError::NotFound`] si el turno no pertenece a la sucursal,
    /// [`IpcError::Conflict`] si existen ventas abiertas, el turno ya está
    /// cerrado o la revisión está obsoleta, y [`IpcError::ValidationError`] con
    /// un importe inválido.
    pub async fn cash_shift_close(
        &self,
        request: CloseCashShiftRequestDto,
    ) -> Result<CashShiftDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let register_id = parse_id(&request.cash_register_id)?;
        let shift_id = parse_id(&request.cash_shift_id)?;
        let expected_version = parse_revision(&request.expected_revision)?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let shift = self
            .cash()?
            .close_cash_shift(
                &principal,
                &mut identities,
                &register_id,
                &shift_id,
                CloseCashShiftRequest {
                    expected_version,
                    closing_declared_amount: request.closing_declared_amount,
                },
                &occurred_at,
            )
            .await?;
        Ok(CashShiftDto::from(&shift))
    }

    fn cash(&self) -> Result<SqliteCashApplication<SqliteOpenSalesForCashShift>, IpcError> {
        let product = self.product_installation()?;
        Ok(SqliteCashApplication::new(
            SqliteUnitOfWork::new(product.pool.clone()),
            SqliteOpenSalesForCashShift,
        ))
    }
}

/// Permisos canónicos que los comandos de caja ejercen.
///
/// Se enumeran aquí para que una prueba de contrato pueda comprobar que el
/// catálogo del sistema los contiene antes de habilitar la superficie IPC.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub(crate) const CASH_PERMISSIONS: [Permission; 6] = [
    Permission::CashRegisterCreate,
    Permission::CashRegisterRead,
    Permission::CashRegisterUpdate,
    Permission::CashShiftOpen,
    Permission::CashShiftRead,
    Permission::CashShiftClose,
];

fn parse_id(value: &str) -> Result<EntityId, IpcError> {
    EntityId::parse(value).map_err(|_| IpcError::ValidationError)
}

fn parse_revision(value: &str) -> Result<u64, IpcError> {
    value.parse::<u64>().map_err(|_| IpcError::ValidationError)
}

fn parse_status(value: &str) -> Result<Status, IpcError> {
    Status::parse(value).map_err(|_| IpcError::ValidationError)
}

fn parse_shift_status(value: &str) -> Result<CashShiftStatus, IpcError> {
    CashShiftStatus::parse(value).map_err(|_| IpcError::ValidationError)
}

impl From<&CashRegister> for CashRegisterDto {
    fn from(register: &CashRegister) -> Self {
        Self {
            id: register.id().as_str().into(),
            branch_id: register.branch_id().as_str().into(),
            code: register.code().as_str().into(),
            name: register.name().as_str().into(),
            status: register.status().as_str().into(),
            revision: register.revision().to_string(),
        }
    }
}

impl From<&CashShift> for CashShiftDto {
    fn from(shift: &CashShift) -> Self {
        Self {
            id: shift.id().as_str().into(),
            cash_register_id: shift.cash_register_id().as_str().into(),
            status: shift.status().as_str().into(),
            opening_amount: shift.opening_amount().to_string(),
            closing_declared_amount: shift
                .closing_declared_amount()
                .map(|amount| amount.to_string()),
            opened_at: shift.opened_at().get().to_string(),
            closed_at: shift.closed_at().map(|at| at.get().to_string()),
            opened_by_user_id: shift.opened_by_user_id().as_str().into(),
            closed_by_user_id: shift
                .closed_by_user_id()
                .map(|user| user.as_str().to_owned()),
            revision: shift.revision().to_string(),
        }
    }
}
