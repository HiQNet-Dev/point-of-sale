//! DTOs de checkout.
//!
//! Entradas y salida del caso de uso; no contienen SQL.

use pos_core::{
    organization_branch::EntityId,
    recipe::ResolvedConsumptionComponent,
    sales::{Payment, PaymentMethod, Sale},
};

/// Pago solicitado por el transporte; los importes se validan en el dominio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckoutPaymentRequest {
    /// Medio de pago canónico.
    pub method: PaymentMethod,
    /// Importe aplicado como decimal exacto de escala 4.
    pub amount: String,
    /// Efectivo entregado; obligatorio para `CASH` y prohibido para `CARD`.
    pub tendered_amount: Option<String>,
}

/// Entrada completa de checkout de una venta abierta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckoutRequest {
    /// Caja del scope operativo.
    pub cash_register_id: EntityId,
    /// Turno OPEN de esa caja.
    pub cash_shift_id: EntityId,
    /// Venta a cobrar.
    pub sale_id: EntityId,
    /// Revisión leída antes de iniciar el checkout.
    pub expected_sale_version: u64,
    /// Pagos declarados por el operador.
    pub payments: Vec<CheckoutPaymentRequest>,
}

/// Resultado de un checkout confirmado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckoutOutcome {
    /// Venta ya `COMPLETED`.
    pub sale: Sale,
    /// Pagos persistidos en orden de posición.
    pub payments: Vec<Payment>,
    /// Consumo total por artículo aplicado al ledger.
    pub consumption: Vec<ResolvedConsumptionComponent>,
}
