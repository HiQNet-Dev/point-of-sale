//! Dominio puro de inventario: artículos, cantidades exactas, saldos y ledger.
//!
//! Sin persistencia ni SQL. Las cantidades usan escala fija 6 y aritmética
//! comprobada; los saldos se derivan del ledger y nunca de aritmética flotante.
//!
//! # Invariantes
//!
//! - Una apertura por sucursal y artículo, siempre antes de cualquier otro
//!   movimiento.
//! - Un ajuste exige motivo y delta distinto de cero.
//! - El consumo de venta es negativo y referencia su venta.
//! - La unidad base puede congelarse, nunca volver a liberarse.
//! - La suficiencia es todo-o-nada bajo política estricta; `ALLOW_NEGATIVE`
//!   permite saldos negativos solo al consumo de ventas.
//!
//! # Organización
//!
//! Por responsabilidad: [`errors`], [`quantity`], [`item`], [`balance`],
//! [`movement`] y [`rules`]. La API pública se reexporta aquí.

/// Escala fija de las cantidades, heredada de `NUMERIC(19,6)`.
pub const QUANTITY_SCALE: u32 = 6;
/// Dígitos enteros máximos de una cantidad.
pub const MAX_QUANTITY_INTEGER_DIGITS: usize = 13;
/// Longitud máxima en unidades UTF-16 del motivo de un movimiento.
pub const MAX_REASON_UNITS: usize = 255;

const MINOR_FACTOR: i128 = 1_000_000;
const MAX_MINOR_MAGNITUDE: i128 = 9_999_999_999_999_999_999;

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

mod balance;
mod errors;
mod item;
mod movement;
mod quantity;
mod rules;

pub use balance::StockBalance;
pub use errors::InventoryError;
pub use item::{
    InventoryItem, InventoryItemPatch, NewInventoryItem, inventory_item_changed_fields,
};
pub use movement::{InventoryMovement, InventoryMovementType, MovementReason, NewMovement};
pub use quantity::{InventoryBaseUnit, Quantity};
pub use rules::{
    ConsumptionRequirement, Shortage, ensure_opening_allowed, ensure_sufficient, shortages,
};
