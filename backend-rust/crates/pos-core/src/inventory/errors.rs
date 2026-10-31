use std::fmt;

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
