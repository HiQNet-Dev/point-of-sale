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
//!
//! # Organización
//!
//! El contexto se divide por responsabilidad: [`errors`], [`state`], [`sale`],
//! [`sale_item`], [`payment`], [`checkout`] y [`pricing`]. La API pública se
//! reexporta aquí para que los consumidores sigan usando `pos_core::sales::*`.

/// Cantidad máxima de unidades por línea, heredada de la validación Java.
pub const MAX_ITEM_QUANTITY: u32 = 1_000_000;
/// Número máximo de pagos admitidos en un checkout.
pub const MAX_PAYMENTS: usize = 10;

mod checkout;
mod errors;
mod payment;
mod pricing;
mod sale;
mod sale_item;
mod state;

pub use checkout::{CheckoutInstruction, validate_checkout};
pub use errors::SaleError;
pub use payment::{NewPayment, Payment, PaymentMethod};
pub use pricing::{
    ModifierGroupAssignment, ModifierPricing, SelectedModifierOption, resolve_modifier_pricing,
};
pub use sale::{NewSale, Sale, SaleScope, total_of_active_items};
pub use sale_item::{NewSaleItem, SaleItem};
pub use state::{SaleItemStatus, SaleStatus};
