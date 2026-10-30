//! DTOs y paginación de inventario.
//!
//! Tipos de aplicación sin SQL.

use pos_core::{inventory::InventoryBaseUnit, organization_branch::EntityId};

/// Tamaño máximo de página para listados de inventario.
pub const MAX_INVENTORY_PAGE_SIZE: u32 = 100;

/// Entrada para crear un artículo de inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInventoryItemRequest {
    /// Código canónico; se normaliza igual que en Java.
    pub code: String,
    /// Nombre visible.
    pub name: String,
    /// Descripción opcional.
    pub description: Option<String>,
    /// Unidad base de todas las cantidades del artículo.
    pub base_unit: InventoryBaseUnit,
    /// Si el artículo admite cantidades fraccionarias.
    pub fractional_unit_allowed: bool,
}

/// Entrada de apertura de existencias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenStockRequest {
    /// Artículo de la organización del principal.
    pub inventory_item_id: EntityId,
    /// Cantidad declarada como existencias iniciales.
    pub quantity: String,
}

/// Entrada de ajuste manual con motivo obligatorio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjustStockRequest {
    /// Artículo de la organización del principal.
    pub inventory_item_id: EntityId,
    /// Delta firmado y distinto de cero.
    pub quantity_delta: String,
    /// Motivo normalizado del ajuste.
    pub reason: String,
}

/// Página determinista de artículos de inventario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryPage {
    /// Máximo de filas, entre uno y [`MAX_INVENTORY_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento en el orden por código e identidad.
    pub offset: u32,
}
