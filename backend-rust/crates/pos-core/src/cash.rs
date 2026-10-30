//! Dominio puro de cajas y turnos de caja.
//!
//! Reproduce `cashregister/domain/CashRegister` y `cashshift/domain/CashShift`
//! sin JPA, Spring, SQLx ni transporte.
//!
//! # Modelo
//!
//! - Una **caja** pertenece a la *sucursal* (`Branch -> CashRegister`), no a la
//!   organización como el catálogo. Su `code` solo es único dentro de la
//!   sucursal y es inmutable.
//! - Un **turno** es un hecho histórico: cada caja acumula un historial
//!   ilimitado de turnos `CLOSED` y puede tener como máximo uno `OPEN`. Esa
//!   regla la garantiza el índice único parcial del esquema físico; aquí solo se
//!   valida el estado y el scope del turno.
//! - Los importes son declaraciones del operador, no conciliaciones: todavía no
//!   existen ventas, pagos ni movimientos de efectivo.
//!
//! # Recuperación
//!
//! Abrir exige sucursal y caja `ACTIVE`. Cerrar **no**: un turno ya abierto debe
//! seguir siendo cerrable aunque la sucursal o la caja se desactiven después.
//! Esta asimetría es deliberada y se expresa en [`ensure_open_allowed`] y
//! [`ensure_close_allowed`].

use std::fmt;

use crate::{
    catalog::{CatalogCode, UnitPrice},
    identity::UtcSeconds,
    organization_branch::{DisplayName, EntityId, Status},
};

/// Fallos de cajas y turnos sin datos sensibles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CashError {
    /// El importe declarado no cumple el contrato `NUMERIC(19,4)`.
    InvalidAmount,
    /// La caja está inactiva y no puede abrir turno.
    RegisterInactive,
    /// La sucursal está inactiva y no puede abrir turno.
    BranchInactive,
    /// La caja ya tiene un turno abierto.
    RegisterAlreadyHasOpenShift,
    /// El turno ya fue cerrado; cerrar de nuevo nunca es una operación nula.
    ShiftAlreadyClosed,
    /// El turno tiene ventas abiertas y no puede cerrarse.
    ShiftHasOpenSales,
    /// La versión esperada no corresponde al turno actual.
    ConcurrentModification,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// La caja no pertenece a la organización o sucursal indicadas.
    OwnershipMismatch,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for CashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAmount => "importe declarado no válido",
            Self::RegisterInactive => "la caja está inactiva",
            Self::BranchInactive => "la sucursal está inactiva",
            Self::RegisterAlreadyHasOpenShift => "la caja ya tiene un turno abierto",
            Self::ShiftAlreadyClosed => "el turno ya está cerrado",
            Self::ShiftHasOpenSales => "el turno tiene ventas abiertas",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::OwnershipMismatch => "la caja pertenece a otra organización o sucursal",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for CashError {}

/// Datos de creación de una caja.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCashRegister {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Sucursal propietaria.
    pub branch_id: EntityId,
    /// Código ya validado; único dentro de la sucursal.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
}

/// Caja de una sucursal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CashRegister {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    status: Status,
    revision: u64,
}

impl CashRegister {
    /// Crea una caja activa con revisión `0`.
    pub fn create(new: NewCashRegister) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            branch_id: new.branch_id,
            code: new.code,
            name: new.name,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata una caja persistida.
    ///
    /// Uso exclusivo de adaptadores: el código y el nombre ya llegan validados.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            branch_id,
            code,
            name,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria, siempre la de la sucursal.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Sucursal propietaria.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    /// Código inmutable, único dentro de la sucursal.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si la caja admite apertura de turno.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Comprueba que la caja pertenece al scope indicado.
    ///
    /// # Errors
    /// Devuelve [`CashError::OwnershipMismatch`] si organización o sucursal no
    /// coinciden.
    pub fn ensure_scope(
        &self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<(), CashError> {
        if &self.organization_id != organization_id || &self.branch_id != branch_id {
            return Err(CashError::OwnershipMismatch);
        }
        Ok(())
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// El código, la organización y la sucursal no son modificables.
    ///
    /// # Errors
    /// Devuelve [`CashError::ConcurrentModification`] o
    /// [`CashError::RevisionOverflow`].
    pub fn patch(&mut self, patch: CashRegisterPatch) -> Result<bool, CashError> {
        if patch.expected_version != self.revision {
            return Err(CashError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(CashError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch de caja: ni el código ni el scope se modifican.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CashRegisterPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Estado de un turno de caja.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CashShiftStatus {
    /// Turno actualmente en operación.
    Open,
    /// Turno histórico e inmutable.
    Closed,
}

impl CashShiftStatus {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Closed => "CLOSED",
        }
    }

    /// Convierte un valor persistido sin aceptar estados desconocidos.
    ///
    /// # Errors
    /// Devuelve [`CashError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, CashError> {
        match value {
            "OPEN" => Ok(Self::Open),
            "CLOSED" => Ok(Self::Closed),
            _ => Err(CashError::InvalidPersistedValue),
        }
    }
}

/// Datos de apertura de un turno.
///
/// El scope (organización y sucursal) se deriva de la caja: un turno nunca puede
/// pertenecer a una organización o sucursal distinta de la de su caja.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCashShift {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Caja ya resuelta en el scope solicitado.
    pub register: CashRegister,
    /// Actor autenticado que abre el turno; nunca proviene del cliente.
    pub opened_by_user_id: EntityId,
    /// Efectivo declarado al abrir.
    pub opening_amount: UnitPrice,
    /// Instante UTC de apertura.
    pub opened_at: UtcSeconds,
}

/// Turno operativo de una caja.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CashShift {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    cash_register_id: EntityId,
    opened_by_user_id: EntityId,
    closed_by_user_id: Option<EntityId>,
    status: CashShiftStatus,
    opening_amount: UnitPrice,
    closing_declared_amount: Option<UnitPrice>,
    opened_at: UtcSeconds,
    closed_at: Option<UtcSeconds>,
    revision: u64,
}

impl CashShift {
    /// Abre un turno sobre una caja activa.
    ///
    /// # Errors
    /// Devuelve [`CashError::RegisterInactive`] si la caja no está activa.
    pub fn open(new: NewCashShift) -> Result<Self, CashError> {
        if !new.register.is_active() {
            return Err(CashError::RegisterInactive);
        }
        Ok(Self {
            id: new.id,
            organization_id: new.register.organization_id().clone(),
            branch_id: new.register.branch_id().clone(),
            cash_register_id: new.register.id().clone(),
            opened_by_user_id: new.opened_by_user_id,
            closed_by_user_id: None,
            status: CashShiftStatus::Open,
            opening_amount: new.opening_amount,
            closing_declared_amount: None,
            opened_at: new.opened_at,
            closed_at: None,
            revision: 0,
        })
    }

    /// Rehidrata un turno persistido validando la coherencia del estado.
    ///
    /// # Errors
    /// Devuelve [`CashError::InvalidPersistedValue`] si `status` y los campos de
    /// cierre no son coherentes.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        cash_register_id: EntityId,
        opened_by_user_id: EntityId,
        closed_by_user_id: Option<EntityId>,
        status: CashShiftStatus,
        opening_amount: UnitPrice,
        closing_declared_amount: Option<UnitPrice>,
        opened_at: UtcSeconds,
        closed_at: Option<UtcSeconds>,
        revision: u64,
    ) -> Result<Self, CashError> {
        let coherent = match status {
            CashShiftStatus::Open => {
                closed_by_user_id.is_none()
                    && closing_declared_amount.is_none()
                    && closed_at.is_none()
            }
            CashShiftStatus::Closed => {
                closed_by_user_id.is_some()
                    && closing_declared_amount.is_some()
                    && closed_at.is_some()
            }
        };
        if !coherent {
            return Err(CashError::InvalidPersistedValue);
        }
        Ok(Self {
            id,
            organization_id,
            branch_id,
            cash_register_id,
            opened_by_user_id,
            closed_by_user_id,
            status,
            opening_amount,
            closing_declared_amount,
            opened_at,
            closed_at,
            revision,
        })
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria, derivada de la caja.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Sucursal propietaria, derivada de la caja.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    /// Caja del turno.
    pub fn cash_register_id(&self) -> &EntityId {
        &self.cash_register_id
    }

    /// Actor que abrió el turno.
    pub fn opened_by_user_id(&self) -> &EntityId {
        &self.opened_by_user_id
    }

    /// Actor que cerró el turno, si ya está cerrado.
    pub fn closed_by_user_id(&self) -> Option<&EntityId> {
        self.closed_by_user_id.as_ref()
    }

    /// Estado vigente.
    pub fn status(&self) -> CashShiftStatus {
        self.status
    }

    /// Efectivo declarado al abrir.
    pub fn opening_amount(&self) -> UnitPrice {
        self.opening_amount
    }

    /// Efectivo declarado al cerrar, si ya está cerrado.
    pub fn closing_declared_amount(&self) -> Option<UnitPrice> {
        self.closing_declared_amount
    }

    /// Instante UTC de apertura.
    pub fn opened_at(&self) -> UtcSeconds {
        self.opened_at
    }

    /// Instante UTC de cierre, si ya está cerrado.
    pub fn closed_at(&self) -> Option<UtcSeconds> {
        self.closed_at
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el turno sigue en operación.
    pub fn is_open(&self) -> bool {
        self.status == CashShiftStatus::Open
    }

    /// Comprueba que el turno pertenece al scope indicado.
    ///
    /// # Errors
    /// Devuelve [`CashError::OwnershipMismatch`].
    pub fn ensure_scope(
        &self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> Result<(), CashError> {
        if &self.organization_id != organization_id
            || &self.branch_id != branch_id
            || &self.cash_register_id != cash_register_id
        {
            return Err(CashError::OwnershipMismatch);
        }
        Ok(())
    }

    /// Cierra el turno declarando el efectivo entregado.
    ///
    /// No es idempotente: cerrar un turno ya cerrado devuelve
    /// [`CashError::ShiftAlreadyClosed`] y nunca modifica el histórico.
    ///
    /// # Errors
    /// Devuelve [`CashError::ShiftAlreadyClosed`],
    /// [`CashError::ConcurrentModification`] o [`CashError::RevisionOverflow`].
    pub fn close(&mut self, request: CloseCashShift) -> Result<(), CashError> {
        if self.status == CashShiftStatus::Closed {
            return Err(CashError::ShiftAlreadyClosed);
        }
        if request.expected_version != self.revision {
            return Err(CashError::ConcurrentModification);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(CashError::RevisionOverflow)?;
        self.closing_declared_amount = Some(request.declared_amount);
        self.closed_by_user_id = Some(request.closed_by_user_id);
        self.closed_at = Some(request.closed_at);
        self.status = CashShiftStatus::Closed;
        self.revision = revision;
        Ok(())
    }
}

/// Datos de cierre de un turno.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseCashShift {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Efectivo físico declarado por el operador.
    pub declared_amount: UnitPrice,
    /// Actor autenticado que cierra el turno; nunca proviene del cliente.
    pub closed_by_user_id: EntityId,
    /// Instante UTC de cierre.
    pub closed_at: UtcSeconds,
}

/// Regla de apertura: sucursal y caja activas y sin turno abierto.
///
/// Replica el orden del servicio Java. La comprobación de turno abierto es una
/// precondición de experiencia: el índice único parcial del esquema físico sigue
/// siendo la autoridad ante una carrera.
///
/// # Errors
/// Devuelve [`CashError::BranchInactive`], [`CashError::RegisterInactive`] o
/// [`CashError::RegisterAlreadyHasOpenShift`].
pub fn ensure_open_allowed(
    branch_is_active: bool,
    register: &CashRegister,
    has_open_shift: bool,
) -> Result<(), CashError> {
    if !branch_is_active {
        return Err(CashError::BranchInactive);
    }
    if !register.is_active() {
        return Err(CashError::RegisterInactive);
    }
    if has_open_shift {
        return Err(CashError::RegisterAlreadyHasOpenShift);
    }
    Ok(())
}

/// Regla de cierre: el turno debe estar abierto y sin ventas abiertas.
///
/// Deliberadamente **no** comprueba el estado activo de la sucursal ni de la
/// caja: un turno abierto debe poder cerrarse aunque el recurso se haya
/// desactivado después, tal como exige el flujo de recuperación de Java.
///
/// # Errors
/// Devuelve [`CashError::ShiftAlreadyClosed`] o [`CashError::ShiftHasOpenSales`].
pub fn ensure_close_allowed(shift: &CashShift, has_open_sales: bool) -> Result<(), CashError> {
    if !shift.is_open() {
        return Err(CashError::ShiftAlreadyClosed);
    }
    if has_open_sales {
        return Err(CashError::ShiftHasOpenSales);
    }
    Ok(())
}

/// Campos modificados de una caja, con los nombres de la auditoría Java.
pub fn cash_register_changed_fields(
    before: &CashRegister,
    after: &CashRegister,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}
