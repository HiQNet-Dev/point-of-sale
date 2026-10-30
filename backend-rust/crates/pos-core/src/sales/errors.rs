use std::fmt;

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
