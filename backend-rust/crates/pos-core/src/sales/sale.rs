use crate::catalog::{Decimal4, UnitPrice};
use crate::identity::UtcSeconds;
use crate::organization_branch::EntityId;

use super::SaleError;
use super::sale_item::SaleItem;
use super::state::SaleStatus;

/// Scope operativo de una venta: organización, sucursal, caja y turno.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaleScope {
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Sucursal del turno.
    pub branch_id: EntityId,
    /// Caja del turno.
    pub cash_register_id: EntityId,
    /// Turno en el que ocurre la venta.
    pub cash_shift_id: EntityId,
}

impl SaleScope {
    /// Comprueba que un scope coincida exactamente.
    ///
    /// # Errors
    /// Devuelve [`SaleError::OwnershipMismatch`].
    pub fn ensure_matches(&self, other: &SaleScope) -> Result<(), SaleError> {
        if self != other {
            return Err(SaleError::OwnershipMismatch);
        }
        Ok(())
    }
}

/// Datos de apertura de una venta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSale {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Scope operativo completo.
    pub scope: SaleScope,
    /// Actor autenticado que abre la venta.
    pub created_by_user_id: EntityId,
}

/// Venta operativa.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sale {
    id: EntityId,
    scope: SaleScope,
    created_by_user_id: EntityId,
    cancelled_by_user_id: Option<EntityId>,
    completed_by_user_id: Option<EntityId>,
    status: SaleStatus,
    total_amount: UnitPrice,
    cancelled_at: Option<UtcSeconds>,
    completed_at: Option<UtcSeconds>,
    revision: u64,
}

impl Sale {
    /// Abre una venta con total `0.0000` y revisión `0`.
    pub fn open(new: NewSale) -> Self {
        Self {
            id: new.id,
            scope: new.scope,
            created_by_user_id: new.created_by_user_id,
            cancelled_by_user_id: None,
            completed_by_user_id: None,
            status: SaleStatus::Open,
            total_amount: UnitPrice::zero(),
            cancelled_at: None,
            completed_at: None,
            revision: 0,
        }
    }

    /// Rehidrata una venta persistida validando la coherencia de su estado.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidPersistedValue`] si el estado no corresponde
    /// a los actores e instantes de cancelación y cobro.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        scope: SaleScope,
        created_by_user_id: EntityId,
        cancelled_by_user_id: Option<EntityId>,
        completed_by_user_id: Option<EntityId>,
        status: SaleStatus,
        total_amount: UnitPrice,
        cancelled_at: Option<UtcSeconds>,
        completed_at: Option<UtcSeconds>,
        revision: u64,
    ) -> Result<Self, SaleError> {
        let coherent = match status {
            SaleStatus::Open => {
                cancelled_by_user_id.is_none()
                    && cancelled_at.is_none()
                    && completed_by_user_id.is_none()
                    && completed_at.is_none()
            }
            SaleStatus::Completed => {
                cancelled_by_user_id.is_none()
                    && cancelled_at.is_none()
                    && completed_by_user_id.is_some()
                    && completed_at.is_some()
            }
            SaleStatus::Cancelled => {
                cancelled_by_user_id.is_some()
                    && cancelled_at.is_some()
                    && completed_by_user_id.is_none()
                    && completed_at.is_none()
            }
        };
        if !coherent {
            return Err(SaleError::InvalidPersistedValue);
        }
        Ok(Self {
            id,
            scope,
            created_by_user_id,
            cancelled_by_user_id,
            completed_by_user_id,
            status,
            total_amount,
            cancelled_at,
            completed_at,
            revision,
        })
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Scope operativo.
    pub fn scope(&self) -> &SaleScope {
        &self.scope
    }
    /// Actor que abrió la venta.
    pub fn created_by_user_id(&self) -> &EntityId {
        &self.created_by_user_id
    }
    /// Actor que canceló la venta, si aplica.
    pub fn cancelled_by_user_id(&self) -> Option<&EntityId> {
        self.cancelled_by_user_id.as_ref()
    }
    /// Actor que cobró la venta, si aplica.
    pub fn completed_by_user_id(&self) -> Option<&EntityId> {
        self.completed_by_user_id.as_ref()
    }
    /// Estado vigente.
    pub fn status(&self) -> SaleStatus {
        self.status
    }
    /// Total de las líneas activas.
    pub fn total_amount(&self) -> UnitPrice {
        self.total_amount
    }
    /// Instante de cancelación, si aplica.
    pub fn cancelled_at(&self) -> Option<UtcSeconds> {
        self.cancelled_at
    }
    /// Instante de cobro, si aplica.
    pub fn completed_at(&self) -> Option<UtcSeconds> {
        self.completed_at
    }
    /// Revisión optimista del agregado.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Exige que la venta esté abierta.
    ///
    /// # Errors
    /// Devuelve [`SaleError::SaleNotOpen`].
    pub fn require_open(&self) -> Result<(), SaleError> {
        if self.status != SaleStatus::Open {
            return Err(SaleError::SaleNotOpen);
        }
        Ok(())
    }

    /// Comprueba el scope operativo.
    ///
    /// # Errors
    /// Devuelve [`SaleError::OwnershipMismatch`].
    pub fn ensure_scope(&self, scope: &SaleScope) -> Result<(), SaleError> {
        self.scope.ensure_matches(scope)
    }

    /// Recalcula el total a partir de las líneas activas.
    ///
    /// Devuelve `false` cuando el total no cambia.
    ///
    /// # Errors
    /// Devuelve [`SaleError::SaleNotOpen`] o [`SaleError::RevisionOverflow`].
    pub fn recalculate_total(&mut self, total: UnitPrice) -> Result<bool, SaleError> {
        self.require_open()?;
        if self.total_amount == total {
            return Ok(false);
        }
        self.total_amount = total;
        self.bump()?;
        Ok(true)
    }

    /// Cancela la venta para recuperación operativa.
    ///
    /// # Errors
    /// Devuelve [`SaleError::SaleAlreadyCancelled`],
    /// [`SaleError::SaleAlreadyCompleted`] o [`SaleError::RevisionOverflow`].
    pub fn cancel(&mut self, actor: EntityId, at: UtcSeconds) -> Result<(), SaleError> {
        if self.status == SaleStatus::Cancelled {
            return Err(SaleError::SaleAlreadyCancelled);
        }
        if self.status == SaleStatus::Completed {
            return Err(SaleError::SaleAlreadyCompleted);
        }
        self.status = SaleStatus::Cancelled;
        self.cancelled_by_user_id = Some(actor);
        self.cancelled_at = Some(at);
        self.bump()
    }

    /// Cobra la venta; solo desde `OPEN`.
    ///
    /// # Errors
    /// Devuelve [`SaleError::SaleNotOpen`] o [`SaleError::RevisionOverflow`].
    pub fn complete(&mut self, actor: EntityId, at: UtcSeconds) -> Result<(), SaleError> {
        self.require_open()?;
        self.status = SaleStatus::Completed;
        self.completed_by_user_id = Some(actor);
        self.completed_at = Some(at);
        self.bump()
    }

    fn bump(&mut self) -> Result<(), SaleError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(SaleError::RevisionOverflow)?;
        Ok(())
    }
}

/// Suma el total de las líneas activas.
///
/// # Errors
/// Devuelve [`SaleError::AmountOutOfRange`] si el total excede el rango.
pub fn total_of_active_items(items: &[SaleItem]) -> Result<UnitPrice, SaleError> {
    let mut total: i128 = 0;
    for item in items.iter().filter(|item| item.is_active()) {
        total = total
            .checked_add(item.line_total().value().as_minor())
            .ok_or(SaleError::AmountOutOfRange)?;
    }
    let amount = Decimal4::from_minor(total).map_err(|_| SaleError::AmountOutOfRange)?;
    UnitPrice::new(amount).map_err(|_| SaleError::AmountOutOfRange)
}
