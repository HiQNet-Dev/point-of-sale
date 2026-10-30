use crate::catalog::{Decimal4, UnitPrice};

use super::payment::PaymentMethod;
use super::{MAX_PAYMENTS, SaleError};

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
