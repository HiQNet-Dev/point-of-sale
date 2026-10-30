use crate::catalog::{Decimal4, UnitPrice};
use crate::organization_branch::EntityId;

use super::SaleError;
use super::sale::SaleScope;

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
