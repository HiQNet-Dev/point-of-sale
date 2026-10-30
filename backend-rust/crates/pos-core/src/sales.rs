//! Dominio puro de ventas, pagos y cotización de modificadores.
//!
//! Reproduce `sales/domain/Sale`, `SaleItem`, `payment/domain/Payment` y las
//! reglas de `SaleService.quote` / `SaleService.checkout` sin JPA, Spring, SQLx
//! ni transporte.
//!
//! # Ciclo de vida
//!
//! ```text
//! OPEN --complete--> COMPLETED
//!   |
//!   +----cancel----> CANCELLED
//! ```
//!
//! Una venta nace `OPEN` con total `0.0000`; las líneas se cotizan y se suman al
//! total. `COMPLETED` y `CANCELLED` son finales. La cancelación no exige padres
//! activos (es recuperación operativa); el checkout sí.
//!
//! # Invariantes
//!
//! - Todo importe es exacto con escala 4 (`NUMERIC(19,4)`), nunca coma flotante.
//! - Cada línea congela SKU, nombre y precios del catálogo; el catálogo no
//!   reescribe el historial.
//! - Solo las líneas `ACTIVE` suman al total; retirar una línea conserva la fila.
//! - El checkout exige al menos una línea activa y que la suma de pagos sea
//!   **exactamente** el total: ni menos, ni más, ni pagos en una venta de cero.
//! - Se admite como máximo un pago en efectivo por venta.
//! - La cotización de modificadores valida pertenencia, actividad, límites de
//!   selección y precio efectivo no negativo.

use std::collections::BTreeSet;
use std::fmt;

use crate::{
    catalog::{Decimal4, PriceDelta, Sku, UnitPrice},
    identity::UtcSeconds,
    organization_branch::{DisplayName, EntityId},
};

/// Cantidad máxima de unidades por línea, heredada de la validación Java.
pub const MAX_ITEM_QUANTITY: u32 = 1_000_000;
/// Número máximo de pagos admitidos en un checkout.
pub const MAX_PAYMENTS: usize = 10;

/// Fallos de ventas, pagos y cotización sin contenido sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaleError {
    /// El importe del pago debe ser estrictamente positivo.
    PaymentMustBePositive,
    /// Un pago en efectivo exige importe entregado.
    CashTenderedRequired,
    /// El efectivo entregado no cubre el importe del pago.
    CashTenderedBelowAmount,
    /// Un pago con tarjeta no admite efectivo entregado.
    CardCannotTender,
    /// Se superó el máximo de pagos por checkout.
    TooManyPayments,
    /// Solo se admite un pago en efectivo por venta.
    MultipleCashPayments,
    /// La suma de pagos no coincide exactamente con el total.
    PaymentTotalMismatch,
    /// La venta no tiene líneas activas.
    NoActiveItems,
    /// La venta ya estaba cancelada.
    SaleAlreadyCancelled,
    /// La venta ya estaba completada.
    SaleAlreadyCompleted,
    /// La venta no está abierta.
    SaleNotOpen,
    /// La línea ya estaba retirada.
    ItemAlreadyRemoved,
    /// La cantidad de la línea está fuera de `1..=1_000_000`.
    InvalidQuantity,
    /// La posición del pago debe ser mayor o igual que 1.
    InvalidPaymentPosition,
    /// El importe o el total de línea excede el rango representable.
    AmountOutOfRange,
    /// La selección repite una opción de modificador.
    DuplicateModifierOption,
    /// La opción seleccionada no pertenece a ningún grupo asignado al producto.
    ModifierOptionNotAssigned,
    /// La opción seleccionada está inactiva.
    ModifierOptionInactive,
    /// Un grupo obligatorio está inactivo.
    RequiredModifierGroupInactive,
    /// Se seleccionaron opciones de un grupo inactivo.
    ModifierGroupInactiveSelection,
    /// El número de selecciones queda fuera de los límites del grupo.
    ModifierSelectionOutOfBounds,
    /// La configuración produce un precio efectivo negativo.
    NegativeEffectivePrice,
    /// La versión esperada no corresponde al agregado.
    ConcurrentModification,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// La entidad pertenece a otro scope operativo.
    OwnershipMismatch,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for SaleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PaymentMustBePositive => "el importe del pago debe ser positivo",
            Self::CashTenderedRequired => "el pago en efectivo exige importe entregado",
            Self::CashTenderedBelowAmount => "el efectivo entregado no cubre el pago",
            Self::CardCannotTender => "el pago con tarjeta no admite efectivo entregado",
            Self::TooManyPayments => "demasiados pagos para un checkout",
            Self::MultipleCashPayments => "solo se admite un pago en efectivo",
            Self::PaymentTotalMismatch => "la suma de pagos no coincide con el total",
            Self::NoActiveItems => "la venta no tiene líneas activas",
            Self::SaleAlreadyCancelled => "la venta ya está cancelada",
            Self::SaleAlreadyCompleted => "la venta ya está completada",
            Self::SaleNotOpen => "la venta no está abierta",
            Self::ItemAlreadyRemoved => "la línea ya está retirada",
            Self::InvalidQuantity => "cantidad de línea no válida",
            Self::InvalidPaymentPosition => "posición de pago no válida",
            Self::AmountOutOfRange => "importe fuera de rango",
            Self::DuplicateModifierOption => "opción de modificador repetida",
            Self::ModifierOptionNotAssigned => "la opción no pertenece al producto",
            Self::ModifierOptionInactive => "la opción está inactiva",
            Self::RequiredModifierGroupInactive => "un grupo obligatorio está inactivo",
            Self::ModifierGroupInactiveSelection => "el grupo está inactivo",
            Self::ModifierSelectionOutOfBounds => "selección fuera de los límites del grupo",
            Self::NegativeEffectivePrice => "el precio efectivo resultante es negativo",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::OwnershipMismatch => "la entidad pertenece a otro scope operativo",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for SaleError {}

/// Estado operativo de una venta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaleStatus {
    /// Venta en construcción.
    Open,
    /// Venta cobrada; estado final.
    Completed,
    /// Venta anulada; estado final.
    Cancelled,
}

impl SaleStatus {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Completed => "COMPLETED",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// Convierte un valor persistido sin aceptar estados desconocidos.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, SaleError> {
        match value {
            "OPEN" => Ok(Self::Open),
            "COMPLETED" => Ok(Self::Completed),
            "CANCELLED" => Ok(Self::Cancelled),
            _ => Err(SaleError::InvalidPersistedValue),
        }
    }
}

/// Estado de una línea de venta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaleItemStatus {
    /// La línea cuenta para el total.
    Active,
    /// La línea se conserva como historial y no cuenta.
    Removed,
}

impl SaleItemStatus {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Removed => "REMOVED",
        }
    }

    /// Convierte un valor persistido sin aceptar estados desconocidos.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, SaleError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "REMOVED" => Ok(Self::Removed),
            _ => Err(SaleError::InvalidPersistedValue),
        }
    }
}

/// Medio de pago local.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentMethod {
    /// Efectivo; admite importe entregado y cambio.
    Cash,
    /// Tarjeta; no admite importe entregado.
    Card,
}

impl PaymentMethod {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "CASH",
            Self::Card => "CARD",
        }
    }

    /// Convierte un valor persistido sin aceptar métodos desconocidos.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, SaleError> {
        match value {
            "CASH" => Ok(Self::Cash),
            "CARD" => Ok(Self::Card),
            _ => Err(SaleError::InvalidPersistedValue),
        }
    }
}

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

/// Datos de creación de una línea.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSaleItem {
    /// Identidad de la línea.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Venta a la que pertenece.
    pub sale_id: EntityId,
    /// Producto vendido.
    pub product_id: EntityId,
    /// SKU congelado.
    pub product_sku_snapshot: Sku,
    /// Nombre congelado.
    pub product_name_snapshot: DisplayName,
    /// Precio base de la organización.
    pub base_unit_price: UnitPrice,
    /// Suma firmada de modificadores.
    pub modifier_unit_delta: PriceDelta,
    /// Precio unitario efectivo, no negativo.
    pub effective_unit_price: UnitPrice,
    /// Cantidad de unidades.
    pub quantity: u32,
    /// Posición dentro de la venta.
    pub position: u32,
}

/// Línea de venta con snapshots inmutables del catálogo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaleItem {
    id: EntityId,
    organization_id: EntityId,
    sale_id: EntityId,
    product_id: EntityId,
    product_sku_snapshot: Sku,
    product_name_snapshot: DisplayName,
    base_unit_price: UnitPrice,
    modifier_unit_delta: PriceDelta,
    effective_unit_price: UnitPrice,
    quantity: u32,
    line_total: UnitPrice,
    status: SaleItemStatus,
    position: u32,
    removed_by_user_id: Option<EntityId>,
    removed_at: Option<UtcSeconds>,
}

impl SaleItem {
    /// Crea una línea activa calculando su total exacto.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidQuantity`] o [`SaleError::AmountOutOfRange`].
    pub fn create(new: NewSaleItem) -> Result<Self, SaleError> {
        validate_quantity(new.quantity)?;
        let line_total = line_total(new.effective_unit_price, new.quantity)?;
        Ok(Self {
            id: new.id,
            organization_id: new.organization_id,
            sale_id: new.sale_id,
            product_id: new.product_id,
            product_sku_snapshot: new.product_sku_snapshot,
            product_name_snapshot: new.product_name_snapshot,
            base_unit_price: new.base_unit_price,
            modifier_unit_delta: new.modifier_unit_delta,
            effective_unit_price: new.effective_unit_price,
            quantity: new.quantity,
            line_total,
            status: SaleItemStatus::Active,
            position: new.position,
            removed_by_user_id: None,
            removed_at: None,
        })
    }

    /// Identidad de la línea.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Venta a la que pertenece.
    pub fn sale_id(&self) -> &EntityId {
        &self.sale_id
    }
    /// Producto vendido.
    pub fn product_id(&self) -> &EntityId {
        &self.product_id
    }
    /// SKU congelado.
    pub fn product_sku_snapshot(&self) -> &Sku {
        &self.product_sku_snapshot
    }
    /// Nombre congelado.
    pub fn product_name_snapshot(&self) -> &DisplayName {
        &self.product_name_snapshot
    }
    /// Precio base.
    pub fn base_unit_price(&self) -> UnitPrice {
        self.base_unit_price
    }
    /// Delta de modificadores.
    pub fn modifier_unit_delta(&self) -> PriceDelta {
        self.modifier_unit_delta
    }
    /// Precio unitario efectivo.
    pub fn effective_unit_price(&self) -> UnitPrice {
        self.effective_unit_price
    }
    /// Cantidad.
    pub fn quantity(&self) -> u32 {
        self.quantity
    }
    /// Total de la línea.
    pub fn line_total(&self) -> UnitPrice {
        self.line_total
    }
    /// Estado de la línea.
    pub fn status(&self) -> SaleItemStatus {
        self.status
    }
    /// Posición dentro de la venta.
    pub fn position(&self) -> u32 {
        self.position
    }
    /// Actor que retiró la línea, si aplica.
    pub fn removed_by_user_id(&self) -> Option<&EntityId> {
        self.removed_by_user_id.as_ref()
    }
    /// Instante de retirada, si aplica.
    pub fn removed_at(&self) -> Option<UtcSeconds> {
        self.removed_at
    }

    /// Indica si la línea cuenta para el total.
    pub fn is_active(&self) -> bool {
        self.status == SaleItemStatus::Active
    }

    /// Exige que la línea siga activa.
    ///
    /// # Errors
    /// Devuelve [`SaleError::ItemAlreadyRemoved`].
    pub fn require_active(&self) -> Result<(), SaleError> {
        if !self.is_active() {
            return Err(SaleError::ItemAlreadyRemoved);
        }
        Ok(())
    }

    /// Reemplaza la cotización de una línea activa.
    ///
    /// # Errors
    /// Devuelve [`SaleError::ItemAlreadyRemoved`],
    /// [`SaleError::InvalidQuantity`] o [`SaleError::AmountOutOfRange`].
    pub fn update_quote(
        &mut self,
        base_unit_price: UnitPrice,
        modifier_unit_delta: PriceDelta,
        effective_unit_price: UnitPrice,
        quantity: u32,
    ) -> Result<(), SaleError> {
        self.require_active()?;
        validate_quantity(quantity)?;
        let total = line_total(effective_unit_price, quantity)?;
        self.base_unit_price = base_unit_price;
        self.modifier_unit_delta = modifier_unit_delta;
        self.effective_unit_price = effective_unit_price;
        self.quantity = quantity;
        self.line_total = total;
        Ok(())
    }

    /// Retira la línea conservando su historial.
    ///
    /// # Errors
    /// Devuelve [`SaleError::ItemAlreadyRemoved`].
    pub fn remove(&mut self, actor: EntityId, at: UtcSeconds) -> Result<(), SaleError> {
        self.require_active()?;
        self.status = SaleItemStatus::Removed;
        self.removed_by_user_id = Some(actor);
        self.removed_at = Some(at);
        Ok(())
    }
}

fn validate_quantity(quantity: u32) -> Result<(), SaleError> {
    if quantity == 0 || quantity > MAX_ITEM_QUANTITY {
        return Err(SaleError::InvalidQuantity);
    }
    Ok(())
}

fn line_total(effective: UnitPrice, quantity: u32) -> Result<UnitPrice, SaleError> {
    let minor = effective
        .value()
        .as_minor()
        .checked_mul(i128::from(quantity))
        .ok_or(SaleError::AmountOutOfRange)?;
    let amount = Decimal4::from_minor(minor).map_err(|_| SaleError::AmountOutOfRange)?;
    UnitPrice::new(amount).map_err(|_| SaleError::AmountOutOfRange)
}

/// Pago inmutable creado únicamente por un checkout exitoso.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Payment {
    id: EntityId,
    scope: SaleScope,
    sale_id: EntityId,
    position: u32,
    method: PaymentMethod,
    amount: UnitPrice,
    tendered_amount: Option<UnitPrice>,
    change_amount: UnitPrice,
    created_by_user_id: EntityId,
}

/// Datos de creación de un pago.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewPayment {
    /// Identidad del pago.
    pub id: EntityId,
    /// Scope operativo de la venta.
    pub scope: SaleScope,
    /// Venta cobrada.
    pub sale_id: EntityId,
    /// Posición del pago, desde 1.
    pub position: u32,
    /// Medio de pago.
    pub method: PaymentMethod,
    /// Importe aplicado al total.
    pub amount: UnitPrice,
    /// Efectivo entregado; solo para `CASH`.
    pub tendered_amount: Option<UnitPrice>,
    /// Actor que registra el pago.
    pub created_by_user_id: EntityId,
}

impl Payment {
    /// Crea un pago validando las reglas de efectivo y tarjeta.
    ///
    /// # Errors
    /// Devuelve [`SaleError::PaymentMustBePositive`],
    /// [`SaleError::CashTenderedRequired`], [`SaleError::CashTenderedBelowAmount`],
    /// [`SaleError::CardCannotTender`] o [`SaleError::AmountOutOfRange`].
    pub fn create(new: NewPayment) -> Result<Self, SaleError> {
        if new.position == 0 {
            return Err(SaleError::InvalidPaymentPosition);
        }
        if new.amount.value().is_zero() || new.amount.value().is_negative() {
            return Err(SaleError::PaymentMustBePositive);
        }
        let (tendered_amount, change_amount) = match new.method {
            PaymentMethod::Cash => {
                let tendered = new.tendered_amount.ok_or(SaleError::CashTenderedRequired)?;
                if tendered.value().is_zero() || tendered.value().is_negative() {
                    return Err(SaleError::CashTenderedRequired);
                }
                if tendered.value() < new.amount.value() {
                    return Err(SaleError::CashTenderedBelowAmount);
                }
                let change = tendered
                    .value()
                    .as_minor()
                    .checked_sub(new.amount.value().as_minor())
                    .and_then(|minor| Decimal4::from_minor(minor).ok())
                    .and_then(|amount| UnitPrice::new(amount).ok())
                    .ok_or(SaleError::AmountOutOfRange)?;
                (Some(tendered), change)
            }
            PaymentMethod::Card => {
                if new.tendered_amount.is_some() {
                    return Err(SaleError::CardCannotTender);
                }
                (None, UnitPrice::zero())
            }
        };
        Ok(Self {
            id: new.id,
            scope: new.scope,
            sale_id: new.sale_id,
            position: new.position,
            method: new.method,
            amount: new.amount,
            tendered_amount,
            change_amount,
            created_by_user_id: new.created_by_user_id,
        })
    }

    /// Identidad del pago.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Scope operativo.
    pub fn scope(&self) -> &SaleScope {
        &self.scope
    }
    /// Venta cobrada.
    pub fn sale_id(&self) -> &EntityId {
        &self.sale_id
    }
    /// Posición del pago.
    pub fn position(&self) -> u32 {
        self.position
    }
    /// Medio de pago.
    pub fn method(&self) -> PaymentMethod {
        self.method
    }
    /// Importe aplicado.
    pub fn amount(&self) -> UnitPrice {
        self.amount
    }
    /// Efectivo entregado.
    pub fn tendered_amount(&self) -> Option<UnitPrice> {
        self.tendered_amount
    }
    /// Cambio devuelto.
    pub fn change_amount(&self) -> UnitPrice {
        self.change_amount
    }
    /// Actor que registró el pago.
    pub fn created_by_user_id(&self) -> &EntityId {
        &self.created_by_user_id
    }
}

/// Instrucción de pago recibida por el checkout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckoutInstruction {
    /// Medio de pago.
    pub method: PaymentMethod,
    /// Importe aplicado.
    pub amount: UnitPrice,
    /// Efectivo entregado; obligatorio en `CASH`.
    pub tendered_amount: Option<UnitPrice>,
}

/// Valida la composición de pagos de un checkout antes de escribir nada.
///
/// Replica el orden de `SaleService.checkout`.
///
/// # Errors
/// Devuelve [`SaleError::NoActiveItems`], [`SaleError::TooManyPayments`],
/// [`SaleError::PaymentTotalMismatch`], [`SaleError::MultipleCashPayments`] o
/// [`SaleError::AmountOutOfRange`].
pub fn validate_checkout(
    total_amount: UnitPrice,
    active_items: usize,
    instructions: &[CheckoutInstruction],
) -> Result<(), SaleError> {
    if active_items == 0 {
        return Err(SaleError::NoActiveItems);
    }
    if instructions.len() > MAX_PAYMENTS {
        return Err(SaleError::TooManyPayments);
    }
    let total_is_zero = total_amount.value().is_zero();
    if total_is_zero && !instructions.is_empty() {
        return Err(SaleError::PaymentTotalMismatch);
    }
    if !total_is_zero && instructions.is_empty() {
        return Err(SaleError::PaymentTotalMismatch);
    }
    if instructions
        .iter()
        .filter(|instruction| instruction.method == PaymentMethod::Cash)
        .count()
        > 1
    {
        return Err(SaleError::MultipleCashPayments);
    }
    let mut applied: i128 = 0;
    for instruction in instructions {
        if instruction.amount.value().is_zero() || instruction.amount.value().is_negative() {
            return Err(SaleError::PaymentMustBePositive);
        }
        applied = applied
            .checked_add(instruction.amount.value().as_minor())
            .ok_or(SaleError::AmountOutOfRange)?;
    }
    let _ = Decimal4::from_minor(applied).map_err(|_| SaleError::AmountOutOfRange)?;
    if applied != total_amount.value().as_minor() {
        return Err(SaleError::PaymentTotalMismatch);
    }
    Ok(())
}

/// Grupo de modificadores asignado al producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierGroupAssignment {
    /// Grupo asignado.
    pub group_id: EntityId,
    /// Estado del grupo.
    pub active: bool,
    /// Mínimo de opciones exigidas.
    pub min_selections: u32,
    /// Máximo de opciones admitidas.
    pub max_selections: u32,
}

/// Opción seleccionada, ya resuelta contra el catálogo de la organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedModifierOption {
    /// Grupo al que pertenece la opción.
    pub group_id: EntityId,
    /// Opción seleccionada.
    pub option_id: EntityId,
    /// Estado de la opción.
    pub active: bool,
    /// Delta firmado de la opción.
    pub price_delta: PriceDelta,
    /// Orden de presentación dentro del grupo.
    pub display_order: u32,
}

/// Resultado de cotizar una configuración de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierPricing {
    /// Suma firmada de deltas.
    pub delta: PriceDelta,
    /// Precio unitario efectivo.
    pub effective_unit_price: UnitPrice,
    /// Opciones seleccionadas en orden canónico `(group_id, option_id)`.
    pub ordered: Vec<(EntityId, EntityId)>,
}

/// Cotiza la configuración de modificadores de un producto.
///
/// Valida pertenencia, actividad y límites por grupo, y produce el orden
/// canónico: grupos según su asignación al producto y opciones por
/// `display_order` y `option_id`.
///
/// # Errors
/// Devuelve [`SaleError::DuplicateModifierOption`],
/// [`SaleError::ModifierOptionNotAssigned`], [`SaleError::ModifierOptionInactive`],
/// [`SaleError::RequiredModifierGroupInactive`],
/// [`SaleError::ModifierGroupInactiveSelection`],
/// [`SaleError::ModifierSelectionOutOfBounds`],
/// [`SaleError::NegativeEffectivePrice`] o [`SaleError::AmountOutOfRange`].
pub fn resolve_modifier_pricing(
    base_price: UnitPrice,
    assignments: &[ModifierGroupAssignment],
    selected: &[SelectedModifierOption],
) -> Result<ModifierPricing, SaleError> {
    let mut seen = BTreeSet::new();
    for option in selected {
        if !seen.insert(&option.option_id) {
            return Err(SaleError::DuplicateModifierOption);
        }
        if !assignments
            .iter()
            .any(|assignment| assignment.group_id == option.group_id)
        {
            return Err(SaleError::ModifierOptionNotAssigned);
        }
        if !option.active {
            return Err(SaleError::ModifierOptionInactive);
        }
    }

    for assignment in assignments {
        let count = selected
            .iter()
            .filter(|option| option.group_id == assignment.group_id)
            .count();
        if !assignment.active {
            if assignment.min_selections > 0 {
                return Err(SaleError::RequiredModifierGroupInactive);
            }
            if count > 0 {
                return Err(SaleError::ModifierGroupInactiveSelection);
            }
        } else if count < assignment.min_selections as usize
            || count > assignment.max_selections as usize
        {
            return Err(SaleError::ModifierSelectionOutOfBounds);
        }
    }

    let mut delta_minor: i128 = 0;
    for option in selected {
        delta_minor = delta_minor
            .checked_add(option.price_delta.value().as_minor())
            .ok_or(SaleError::AmountOutOfRange)?;
    }
    let delta_amount =
        Decimal4::from_minor(delta_minor).map_err(|_| SaleError::AmountOutOfRange)?;
    let effective_minor = base_price
        .value()
        .as_minor()
        .checked_add(delta_minor)
        .ok_or(SaleError::AmountOutOfRange)?;
    if effective_minor < 0 {
        return Err(SaleError::NegativeEffectivePrice);
    }
    let effective_amount =
        Decimal4::from_minor(effective_minor).map_err(|_| SaleError::AmountOutOfRange)?;
    let effective_unit_price =
        UnitPrice::new(effective_amount).map_err(|_| SaleError::NegativeEffectivePrice)?;

    let mut ordered = Vec::new();
    for assignment in assignments {
        let mut options: Vec<&SelectedModifierOption> = selected
            .iter()
            .filter(|option| option.group_id == assignment.group_id)
            .collect();
        options.sort_by(|left, right| {
            left.display_order
                .cmp(&right.display_order)
                .then_with(|| left.option_id.cmp(&right.option_id))
        });
        for option in options {
            ordered.push((assignment.group_id.clone(), option.option_id.clone()));
        }
    }

    Ok(ModifierPricing {
        delta: PriceDelta::new(delta_amount),
        effective_unit_price,
        ordered,
    })
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
