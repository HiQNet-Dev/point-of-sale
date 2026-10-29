//! Dominio puro de inventario: artículos, cantidades exactas, balances y ledger.
//!
//! Reproduce `inventory/domain/InventoryItem`, `inventory/stock/domain/*` y las
//! reglas de `InventoryStockService` / `InventorySaleConsumptionService` sin JPA,
//! Spring, SQLx ni transporte.
//!
//! # Invariantes
//!
//! - La **unidad base** es inmutable una vez que el artículo tiene dependencias:
//!   [`InventoryItem::mark_unit_locked`] es monótono y
//!   [`InventoryItem::patch`] rechaza el cambio cuando está bloqueada.
//! - Las cantidades son decimales exactos de escala 6 (`NUMERIC(19,6)`), nunca
//!   coma flotante, y se interpretan siempre como texto decimal.
//! - Un artículo `UNIT` sin `fractional_unit_allowed` no admite cantidades
//!   fraccionarias; la comprobación se aplica al saldo **resultante**.
//! - El **ledger** es append-only: cada movimiento declara su delta, su saldo
//!   resultante y el actor autenticado. Los tipos `SALE_CONSUMPTION` y
//!   `PURCHASE_RECEIPT` exigen su referencia de origen.
//! - La política de stock negativo solo afecta al consumo de ventas:
//!   [`ensure_sufficient`] es todo-o-nada bajo `STRICT` y permisiva bajo
//!   `ALLOW_NEGATIVE`. Nunca cambia la apertura ni los ajustes manuales.
//!
//! # Límite conocido
//!
//! `NUMERIC(19,6)` admite magnitudes que no caben en un `INTEGER` de SQLite.
//! [`Quantity::as_minor_i64`] informa esa frontera; la representación física del
//! inventario Local sigue pendiente de aprobación.

use std::fmt;

use crate::{
    catalog::{CatalogCode, Description, PatchField},
    identity::UtcSeconds,
    organization_branch::{DisplayName, EntityId, NegativeStockPolicy, Status},
};

/// Escala decimal del contrato `NUMERIC(19,6)`.
pub const QUANTITY_SCALE: u32 = 6;
/// Dígitos enteros máximos del contrato `NUMERIC(19,6)`.
pub const MAX_QUANTITY_INTEGER_DIGITS: usize = 13;
/// Longitud máxima en unidades UTF-16 del motivo de un movimiento.
pub const MAX_REASON_UNITS: usize = 255;

const MINOR_FACTOR: i128 = 1_000_000;
const MAX_MINOR_MAGNITUDE: i128 = 9_999_999_999_999_999_999;

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Fallos de inventario sin contenido sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryError {
    /// La cantidad no cumple la sintaxis o el rango de `NUMERIC(19,6)`.
    InvalidQuantity,
    /// La suma de cantidades excede el rango representable.
    QuantityOverflow,
    /// La unidad base ya está bloqueada y no puede cambiarse.
    UnitLocked,
    /// El artículo no admite cantidades fraccionarias.
    FractionalQuantityNotAllowed,
    /// El motivo de un ajuste es obligatorio.
    ReasonRequired,
    /// El motivo supera el límite.
    ReasonTooLong,
    /// La apertura ya existe para el artículo en esa sucursal.
    OpeningAlreadyExists,
    /// La apertura solo se admite antes de cualquier otro movimiento.
    HistoryAlreadyExists,
    /// Una apertura no puede ser negativa.
    OpeningMustBeNonNegative,
    /// Un ajuste no puede ser nulo.
    AdjustmentMustNotBeZero,
    /// El consumo de venta debe ser negativo.
    SaleConsumptionMustBeNegative,
    /// El consumo de venta exige la venta de origen.
    SaleConsumptionRequiresSale,
    /// La recepción de compra debe ser positiva.
    PurchaseReceiptMustBePositive,
    /// La recepción de compra exige la línea de origen.
    PurchaseReceiptRequiresLine,
    /// El movimiento lleva una referencia de origen que su tipo no admite.
    UnexpectedMovementReference,
    /// La versión esperada no corresponde al balance.
    ConcurrentModification,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// No hay existencias suficientes para completar el consumo.
    InsufficientInventory,
    /// El balance no pertenece al scope indicado.
    OwnershipMismatch,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for InventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuantity => "cantidad no válida",
            Self::QuantityOverflow => "la cantidad excede el rango representable",
            Self::UnitLocked => "la unidad base está bloqueada",
            Self::FractionalQuantityNotAllowed => "el artículo no admite cantidades fraccionarias",
            Self::ReasonRequired => "el motivo es obligatorio",
            Self::ReasonTooLong => "el motivo excede el límite",
            Self::OpeningAlreadyExists => "la apertura ya existe",
            Self::HistoryAlreadyExists => "la apertura exige un historial vacío",
            Self::OpeningMustBeNonNegative => "la apertura no puede ser negativa",
            Self::AdjustmentMustNotBeZero => "el ajuste no puede ser nulo",
            Self::SaleConsumptionMustBeNegative => "el consumo de venta debe ser negativo",
            Self::SaleConsumptionRequiresSale => "el consumo exige la venta de origen",
            Self::PurchaseReceiptMustBePositive => "la recepción debe ser positiva",
            Self::PurchaseReceiptRequiresLine => "la recepción exige la línea de origen",
            Self::UnexpectedMovementReference => "referencia de origen no admitida por el tipo",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::InsufficientInventory => "existencias insuficientes",
            Self::OwnershipMismatch => "el balance pertenece a otro scope",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for InventoryError {}

/// Unidad base de un artículo de inventario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryBaseUnit {
    /// Unidad discreta.
    Unit,
    /// Gramo.
    Gram,
    /// Mililitro.
    Milliliter,
}

impl InventoryBaseUnit {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unit => "UNIT",
            Self::Gram => "GRAM",
            Self::Milliliter => "MILLILITER",
        }
    }

    /// Convierte un valor persistido sin aceptar unidades desconocidas.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, InventoryError> {
        match value {
            "UNIT" => Ok(Self::Unit),
            "GRAM" => Ok(Self::Gram),
            "MILLILITER" => Ok(Self::Milliliter),
            _ => Err(InventoryError::InvalidPersistedValue),
        }
    }
}

/// Cantidad exacta con escala fija 6.
///
/// Sintaxis admitida: signo `-` opcional, uno o más dígitos enteros y, si
/// aparece el punto, al menos un decimal. Rechaza `+`, exponentes, espacios y
/// más de seis decimales, incluidos los ceros finales, igual que
/// `InventoryQuantity.parse`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Quantity(i128);

impl Quantity {
    /// Interpreta una cantidad decimal canónica.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidQuantity`] si la sintaxis o el rango no
    /// corresponden a `NUMERIC(19,6)`.
    pub fn parse(input: &str) -> Result<Self, InventoryError> {
        let (negative, unsigned) = match input.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, input),
        };
        let (integer, fraction) = match unsigned.split_once('.') {
            Some((integer, fraction)) => {
                if fraction.is_empty() {
                    return Err(InventoryError::InvalidQuantity);
                }
                (integer, fraction)
            }
            None => (unsigned, ""),
        };
        if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InventoryError::InvalidQuantity);
        }
        if fraction.len() > QUANTITY_SCALE as usize
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(InventoryError::InvalidQuantity);
        }
        let significant = integer.trim_start_matches('0');
        if significant.len() > MAX_QUANTITY_INTEGER_DIGITS {
            return Err(InventoryError::InvalidQuantity);
        }
        let mut magnitude = 0_i128;
        for byte in integer.bytes() {
            magnitude = magnitude * 10 + i128::from(byte - b'0');
        }
        magnitude *= MINOR_FACTOR;
        let mut scaled_fraction = 0_i128;
        for byte in fraction.bytes() {
            scaled_fraction = scaled_fraction * 10 + i128::from(byte - b'0');
        }
        for _ in fraction.len()..QUANTITY_SCALE as usize {
            scaled_fraction *= 10;
        }
        magnitude += scaled_fraction;
        if magnitude > MAX_MINOR_MAGNITUDE {
            return Err(InventoryError::InvalidQuantity);
        }
        Ok(Self(if negative { -magnitude } else { magnitude }))
    }

    /// Construye desde unidades menores ya expresadas con escala 6.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidQuantity`] fuera de rango.
    pub fn from_minor(minor: i128) -> Result<Self, InventoryError> {
        if minor.unsigned_abs() > MAX_MINOR_MAGNITUDE.unsigned_abs() {
            return Err(InventoryError::InvalidQuantity);
        }
        Ok(Self(minor))
    }

    /// Cantidad cero.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Unidades menores con escala 6.
    pub fn as_minor(&self) -> i128 {
        self.0
    }

    /// Unidades menores si caben en el `INTEGER` de SQLite.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidQuantity`] fuera del rango de `i64`.
    pub fn as_minor_i64(&self) -> Result<i64, InventoryError> {
        i64::try_from(self.0).map_err(|_| InventoryError::InvalidQuantity)
    }

    /// Indica si la cantidad es estrictamente negativa.
    pub fn is_negative(&self) -> bool {
        self.0 < 0
    }

    /// Indica si la cantidad es exactamente cero.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Indica si la cantidad tiene parte fraccionaria distinta de cero.
    pub fn is_fractional(&self) -> bool {
        self.0 % MINOR_FACTOR != 0
    }

    /// Suma exacta con verificación de rango.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::QuantityOverflow`].
    pub fn checked_add(&self, other: &Self) -> Result<Self, InventoryError> {
        let sum = self
            .0
            .checked_add(other.0)
            .ok_or(InventoryError::QuantityOverflow)?;
        Self::from_minor(sum).map_err(|_| InventoryError::QuantityOverflow)
    }

    /// Devuelve el opuesto exacto.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::QuantityOverflow`] si no es representable.
    pub fn checked_neg(&self) -> Result<Self, InventoryError> {
        let negated = self
            .0
            .checked_neg()
            .ok_or(InventoryError::QuantityOverflow)?;
        Self::from_minor(negated).map_err(|_| InventoryError::QuantityOverflow)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let integer = magnitude / MINOR_FACTOR.unsigned_abs();
        let fraction = magnitude % MINOR_FACTOR.unsigned_abs();
        if self.0 < 0 {
            formatter.write_str("-")?;
        }
        write!(formatter, "{integer}.{fraction:06}")
    }
}

/// Motivo normalizado de un ajuste o apertura.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MovementReason(String);

impl MovementReason {
    /// Normaliza el motivo; exige contenido y respeta el límite.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::ReasonRequired`] si queda vacío y
    /// [`InventoryError::ReasonTooLong`] si supera el límite.
    pub fn new(value: &str) -> Result<Self, InventoryError> {
        let trimmed = value.trim_matches(|character: char| character <= '\u{20}');
        if trimmed.is_empty() {
            return Err(InventoryError::ReasonRequired);
        }
        if utf16_len(trimmed) > MAX_REASON_UNITS {
            return Err(InventoryError::ReasonTooLong);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`] si no lo cumple.
    pub fn restore(value: &str) -> Result<Self, InventoryError> {
        match Self::new(value) {
            Ok(parsed) if parsed.as_str() == value => Ok(parsed),
            _ => Err(InventoryError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Datos de creación de un artículo de inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInventoryItem {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Código ya validado; único por organización e inmutable.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
    /// Descripción opcional ya normalizada.
    pub description: Option<Description>,
    /// Unidad base de todas las cantidades del artículo.
    pub base_unit: InventoryBaseUnit,
    /// Si el artículo admite cantidades fraccionarias.
    pub fractional_unit_allowed: bool,
}

/// Artículo de inventario propiedad de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryItem {
    id: EntityId,
    organization_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    description: Option<Description>,
    base_unit: InventoryBaseUnit,
    fractional_unit_allowed: bool,
    unit_locked: bool,
    status: Status,
    revision: u64,
}

impl InventoryItem {
    /// Crea un artículo activo, con unidad libre y revisión `0`.
    pub fn create(new: NewInventoryItem) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            code: new.code,
            name: new.name,
            description: new.description,
            base_unit: new.base_unit,
            fractional_unit_allowed: new.fractional_unit_allowed,
            unit_locked: false,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata un artículo persistido.
    ///
    /// Uso exclusivo de adaptadores: los valores ya llegan validados.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        description: Option<Description>,
        base_unit: InventoryBaseUnit,
        fractional_unit_allowed: bool,
        unit_locked: bool,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            code,
            name,
            description,
            base_unit,
            fractional_unit_allowed,
            unit_locked,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Código inmutable y único por organización.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Descripción opcional.
    pub fn description(&self) -> Option<&Description> {
        self.description.as_ref()
    }

    /// Unidad base de las cantidades.
    pub fn base_unit(&self) -> InventoryBaseUnit {
        self.base_unit
    }

    /// Indica si el artículo admite cantidades fraccionarias.
    pub fn fractional_unit_allowed(&self) -> bool {
        self.fractional_unit_allowed
    }

    /// Indica si la unidad base quedó congelada por dependencias.
    pub fn unit_locked(&self) -> bool {
        self.unit_locked
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el artículo admite mutaciones de stock.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Congela la unidad base de forma monótona.
    ///
    /// No incrementa la revisión: es un marcador persistido por una sentencia
    /// dedicada, no una modificación de negocio. Nunca vuelve a `false`.
    pub fn mark_unit_locked(&mut self) -> bool {
        let changed = !self.unit_locked;
        self.unit_locked = true;
        changed
    }

    /// Comprueba que la unidad admita la cantidad indicada.
    ///
    /// Un artículo `UNIT` sin ``fractional_unit_allowed`` exige cantidad entera.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::FractionalQuantityNotAllowed`].
    pub fn ensure_quantity_fits_unit(&self, quantity: Quantity) -> Result<(), InventoryError> {
        if self.base_unit == InventoryBaseUnit::Unit
            && !self.fractional_unit_allowed
            && quantity.is_fractional()
        {
            return Err(InventoryError::FractionalQuantityNotAllowed);
        }
        Ok(())
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// El código y la organización no son modificables. Cambiar la unidad base
    /// exige que siga libre.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::UnitLocked`],
    /// [`InventoryError::ConcurrentModification`] o
    /// [`InventoryError::RevisionOverflow`].
    pub fn patch(&mut self, patch: InventoryItemPatch) -> Result<bool, InventoryError> {
        if patch.expected_version != self.revision {
            return Err(InventoryError::ConcurrentModification);
        }
        if let Some(base_unit) = patch.base_unit
            && base_unit != self.base_unit
            && self.unit_locked
        {
            return Err(InventoryError::UnitLocked);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        match patch.description {
            PatchField::Absent => {}
            PatchField::Clear => {
                changed |= next.description.is_some();
                next.description = None;
            }
            PatchField::Set(value) => {
                changed |= next.description.as_ref() != Some(&value);
                next.description = Some(value);
            }
        }
        if let Some(value) = patch.base_unit {
            changed |= next.base_unit != value;
            next.base_unit = value;
        }
        if let Some(value) = patch.fractional_unit_allowed {
            changed |= next.fractional_unit_allowed != value;
            next.fractional_unit_allowed = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(InventoryError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch de artículo de inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryItemPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Descripción: ausente, limpiada o nueva.
    pub description: PatchField<Description>,
    /// Unidad base nueva; solo si sigue libre.
    pub base_unit: Option<InventoryBaseUnit>,
    /// Autorización de fracciones nueva.
    pub fractional_unit_allowed: Option<bool>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Saldo materializado de un artículo en una sucursal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockBalance {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    inventory_item_id: EntityId,
    quantity_on_hand: Quantity,
    revision: u64,
}

impl StockBalance {
    /// Saldo inicial en cero con revisión `0`.
    ///
    /// Una fila ausente se lee como cero; este constructor modela esa fila.
    pub fn empty(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        inventory_item_id: EntityId,
    ) -> Self {
        Self {
            id,
            organization_id,
            branch_id,
            inventory_item_id,
            quantity_on_hand: Quantity::zero(),
            revision: 0,
        }
    }

    /// Rehidrata un saldo persistido.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        inventory_item_id: EntityId,
        quantity_on_hand: Quantity,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            branch_id,
            inventory_item_id,
            quantity_on_hand,
            revision,
        }
    }

    /// Identidad de la fila de saldo.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Sucursal del saldo.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    /// Artículo del saldo.
    pub fn inventory_item_id(&self) -> &EntityId {
        &self.inventory_item_id
    }

    /// Cantidad vigente.
    pub fn quantity_on_hand(&self) -> Quantity {
        self.quantity_on_hand
    }

    /// Revisión optimista del saldo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Comprueba la pertenencia al scope indicado.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::OwnershipMismatch`].
    pub fn ensure_scope(
        &self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> Result<(), InventoryError> {
        if &self.organization_id != organization_id
            || &self.branch_id != branch_id
            || &self.inventory_item_id != inventory_item_id
        {
            return Err(InventoryError::OwnershipMismatch);
        }
        Ok(())
    }

    /// Aplica un delta bajo CAS y devuelve el saldo resultante.
    ///
    /// No aplica la regla de unidad ni la política de stock negativo: esas
    /// comprobaciones las realizan [`InventoryItem::ensure_quantity_fits_unit`] y
    /// [`ensure_sufficient`] en la misma decisión de negocio.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::ConcurrentModification`],
    /// [`InventoryError::QuantityOverflow`] o [`InventoryError::RevisionOverflow`].
    pub fn apply_delta(
        &mut self,
        expected_version: u64,
        delta: Quantity,
    ) -> Result<Quantity, InventoryError> {
        if expected_version != self.revision {
            return Err(InventoryError::ConcurrentModification);
        }
        let resulting = self.quantity_on_hand.checked_add(&delta)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(InventoryError::RevisionOverflow)?;
        self.quantity_on_hand = resulting;
        self.revision = revision;
        Ok(resulting)
    }
}

/// Tipo de movimiento del ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryMovementType {
    /// Apertura inicial de existencias (`>= 0`, una por sucursal y artículo).
    Opening,
    /// Ajuste manual (`<> 0`, motivo obligatorio).
    Adjustment,
    /// Consumo por venta (`< 0`, venta obligatoria).
    SaleConsumption,
    /// Recepción de compra (`> 0`, línea de recepción obligatoria).
    PurchaseReceipt,
}

impl InventoryMovementType {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opening => "OPENING",
            Self::Adjustment => "ADJUSTMENT",
            Self::SaleConsumption => "SALE_CONSUMPTION",
            Self::PurchaseReceipt => "PURCHASE_RECEIPT",
        }
    }

    /// Convierte un valor persistido sin aceptar tipos desconocidos.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, InventoryError> {
        match value {
            "OPENING" => Ok(Self::Opening),
            "ADJUSTMENT" => Ok(Self::Adjustment),
            "SALE_CONSUMPTION" => Ok(Self::SaleConsumption),
            "PURCHASE_RECEIPT" => Ok(Self::PurchaseReceipt),
            _ => Err(InventoryError::InvalidPersistedValue),
        }
    }
}

/// Hecho inmutable del ledger de existencias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryMovement {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    inventory_item_id: EntityId,
    movement_type: InventoryMovementType,
    quantity_delta: Quantity,
    resulting_quantity: Quantity,
    reason: Option<MovementReason>,
    actor_user_id: EntityId,
    occurred_at: UtcSeconds,
    sale_id: Option<EntityId>,
    purchase_receipt_line_id: Option<EntityId>,
}

/// Datos comunes de un movimiento.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMovement {
    /// Identidad del hecho.
    pub id: EntityId,
    /// Organización del scope.
    pub organization_id: EntityId,
    /// Sucursal del scope.
    pub branch_id: EntityId,
    /// Artículo afectado.
    pub inventory_item_id: EntityId,
    /// Actor autenticado; nunca proviene del cliente.
    pub actor_user_id: EntityId,
    /// Instante UTC del hecho.
    pub occurred_at: UtcSeconds,
}

impl InventoryMovement {
    /// Registra una apertura de existencias.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::OpeningMustBeNonNegative`].
    pub fn opening(
        new: NewMovement,
        quantity: Quantity,
        resulting_quantity: Quantity,
    ) -> Result<Self, InventoryError> {
        if quantity.is_negative() {
            return Err(InventoryError::OpeningMustBeNonNegative);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::Opening,
            quantity,
            resulting_quantity,
            None,
            None,
            None,
        ))
    }

    /// Registra un ajuste manual.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::AdjustmentMustNotBeZero`] si el delta es nulo.
    pub fn adjustment(
        new: NewMovement,
        delta: Quantity,
        resulting_quantity: Quantity,
        reason: MovementReason,
    ) -> Result<Self, InventoryError> {
        if delta.is_zero() {
            return Err(InventoryError::AdjustmentMustNotBeZero);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::Adjustment,
            delta,
            resulting_quantity,
            Some(reason),
            None,
            None,
        ))
    }

    /// Registra el consumo de una venta.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::SaleConsumptionMustBeNegative`] si el delta no
    /// es negativo.
    pub fn sale_consumption(
        new: NewMovement,
        delta: Quantity,
        resulting_quantity: Quantity,
        sale_id: EntityId,
    ) -> Result<Self, InventoryError> {
        if !delta.is_negative() {
            return Err(InventoryError::SaleConsumptionMustBeNegative);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::SaleConsumption,
            delta,
            resulting_quantity,
            None,
            Some(sale_id),
            None,
        ))
    }

    /// Registra la recepción de una línea de compra.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::PurchaseReceiptMustBePositive`] si el delta no
    /// es positivo.
    pub fn purchase_receipt(
        new: NewMovement,
        delta: Quantity,
        resulting_quantity: Quantity,
        receipt_line_id: EntityId,
    ) -> Result<Self, InventoryError> {
        if delta.is_negative() || delta.is_zero() {
            return Err(InventoryError::PurchaseReceiptMustBePositive);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::PurchaseReceipt,
            delta,
            resulting_quantity,
            None,
            None,
            Some(receipt_line_id),
        ))
    }

    fn build(
        new: NewMovement,
        movement_type: InventoryMovementType,
        quantity_delta: Quantity,
        resulting_quantity: Quantity,
        reason: Option<MovementReason>,
        sale_id: Option<EntityId>,
        purchase_receipt_line_id: Option<EntityId>,
    ) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            branch_id: new.branch_id,
            inventory_item_id: new.inventory_item_id,
            movement_type,
            quantity_delta,
            resulting_quantity,
            reason,
            actor_user_id: new.actor_user_id,
            occurred_at: new.occurred_at,
            sale_id,
            purchase_receipt_line_id,
        }
    }

    /// Rehidrata un movimiento persistido validando la coherencia de su tipo.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`] si el delta, el motivo
    /// o las referencias no corresponden al tipo.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        inventory_item_id: EntityId,
        movement_type: InventoryMovementType,
        quantity_delta: Quantity,
        resulting_quantity: Quantity,
        reason: Option<MovementReason>,
        actor_user_id: EntityId,
        occurred_at: UtcSeconds,
        sale_id: Option<EntityId>,
        purchase_receipt_line_id: Option<EntityId>,
    ) -> Result<Self, InventoryError> {
        let coherent = match movement_type {
            InventoryMovementType::Opening => {
                !quantity_delta.is_negative()
                    && reason.is_none()
                    && sale_id.is_none()
                    && purchase_receipt_line_id.is_none()
            }
            InventoryMovementType::Adjustment => {
                !quantity_delta.is_zero()
                    && reason.is_some()
                    && sale_id.is_none()
                    && purchase_receipt_line_id.is_none()
            }
            InventoryMovementType::SaleConsumption => {
                quantity_delta.is_negative()
                    && reason.is_none()
                    && sale_id.is_some()
                    && purchase_receipt_line_id.is_none()
            }
            InventoryMovementType::PurchaseReceipt => {
                !quantity_delta.is_negative()
                    && !quantity_delta.is_zero()
                    && reason.is_none()
                    && sale_id.is_none()
                    && purchase_receipt_line_id.is_some()
            }
        };
        if !coherent {
            return Err(InventoryError::InvalidPersistedValue);
        }
        Ok(Self {
            id,
            organization_id,
            branch_id,
            inventory_item_id,
            movement_type,
            quantity_delta,
            resulting_quantity,
            reason,
            actor_user_id,
            occurred_at,
            sale_id,
            purchase_receipt_line_id,
        })
    }

    /// Identidad del hecho.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización del scope.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Sucursal del scope.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }
    /// Artículo afectado.
    pub fn inventory_item_id(&self) -> &EntityId {
        &self.inventory_item_id
    }
    /// Tipo del movimiento.
    pub fn movement_type(&self) -> InventoryMovementType {
        self.movement_type
    }
    /// Delta aplicado al saldo.
    pub fn quantity_delta(&self) -> Quantity {
        self.quantity_delta
    }
    /// Saldo resultante declarado.
    pub fn resulting_quantity(&self) -> Quantity {
        self.resulting_quantity
    }
    /// Motivo, obligatorio en ajustes.
    pub fn reason(&self) -> Option<&MovementReason> {
        self.reason.as_ref()
    }
    /// Actor autenticado.
    pub fn actor_user_id(&self) -> &EntityId {
        &self.actor_user_id
    }
    /// Instante UTC del hecho.
    pub fn occurred_at(&self) -> UtcSeconds {
        self.occurred_at
    }
    /// Venta de origen del consumo.
    pub fn sale_id(&self) -> Option<&EntityId> {
        self.sale_id.as_ref()
    }
    /// Línea de recepción de origen.
    pub fn purchase_receipt_line_id(&self) -> Option<&EntityId> {
        self.purchase_receipt_line_id.as_ref()
    }
}

/// Requisito de consumo de una venta para un artículo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumptionRequirement {
    /// Artículo requerido.
    pub inventory_item_id: EntityId,
    /// Código del artículo al momento de la venta (para el mensaje de faltante).
    pub inventory_item_code: CatalogCode,
    /// Cantidad requerida, siempre positiva.
    pub required_quantity: Quantity,
}

/// Faltante detectado bajo política estricta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shortage {
    /// Código del artículo.
    pub inventory_item_code: CatalogCode,
    /// Cantidad requerida.
    pub required: Quantity,
    /// Cantidad disponible.
    pub available: Quantity,
}

/// Calcula los faltantes de un consumo bajo política estricta.
///
/// El orden de salida es el de `requirements`, igual que el servicio Java.
pub fn shortages(
    balances: &[(EntityId, Quantity)],
    requirements: &[ConsumptionRequirement],
) -> Vec<Shortage> {
    let mut found = Vec::new();
    for requirement in requirements {
        let available = balances
            .iter()
            .find(|(id, _)| id == &requirement.inventory_item_id)
            .map_or_else(Quantity::zero, |(_, quantity)| *quantity);
        if available < requirement.required_quantity {
            found.push(Shortage {
                inventory_item_code: requirement.inventory_item_code.clone(),
                required: requirement.required_quantity,
                available,
            });
        }
    }
    found
}

/// Regla de suficiencia del consumo de ventas.
///
/// Bajo `STRICT` el consumo es todo-o-nada; bajo `ALLOW_NEGATIVE` el saldo puede
/// quedar negativo. La política nunca se aplica a aperturas ni ajustes.
///
/// # Errors
/// Devuelve [`InventoryError::InsufficientInventory`] si falta existencias bajo
/// `STRICT`.
pub fn ensure_sufficient(
    policy: NegativeStockPolicy,
    balances: &[(EntityId, Quantity)],
    requirements: &[ConsumptionRequirement],
) -> Result<(), InventoryError> {
    if policy == NegativeStockPolicy::AllowNegative {
        return Ok(());
    }
    if shortages(balances, requirements).is_empty() {
        Ok(())
    } else {
        Err(InventoryError::InsufficientInventory)
    }
}

/// Regla de apertura: una sola apertura y antes de cualquier otro movimiento.
///
/// # Errors
/// Devuelve [`InventoryError::OpeningAlreadyExists`] o
/// [`InventoryError::HistoryAlreadyExists`], en el orden del servicio Java.
pub fn ensure_opening_allowed(
    has_opening: bool,
    has_any_movement: bool,
) -> Result<(), InventoryError> {
    if has_opening {
        return Err(InventoryError::OpeningAlreadyExists);
    }
    if has_any_movement {
        return Err(InventoryError::HistoryAlreadyExists);
    }
    Ok(())
}

/// Campos modificados de un artículo, con los nombres de la auditoría Java.
pub fn inventory_item_changed_fields(
    before: &InventoryItem,
    after: &InventoryItem,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.description != after.description {
        changed.push("description");
    }
    if before.base_unit != after.base_unit {
        changed.push("baseUnit");
    }
    if before.fractional_unit_allowed != after.fractional_unit_allowed {
        changed.push("fractionalUnitAllowed");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}
