//! DTOs y snapshots de aplicación de ventas.
//!
//! No contienen SQL; el adaptador los consume y los mapea.

use pos_core::{catalog::PriceDelta, organization_branch::EntityId};

/// Tamaño máximo de página para el historial de ventas de un turno.
pub const MAX_SALE_PAGE_SIZE: u32 = 100;

#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct ModifierSnapshot {
    pub group_id: EntityId,
    pub option_id: EntityId,
    pub group_code: String,
    pub group_name: String,
    pub option_code: String,
    pub option_name: String,
    pub price_delta: PriceDelta,
    pub active: bool,
    pub display_order: u32,
}

/// Entrada para abrir una venta; el scope procede del principal y los IDs de caja/turno.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSaleRequest {
    /// Caja activa dentro de la sucursal del principal.
    pub cash_register_id: EntityId,
    /// Turno OPEN perteneciente a la caja indicada.
    pub cash_shift_id: EntityId,
}

/// Entrada para cotizar una partida; precios y snapshots son exclusivamente del servidor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddSaleItemRequest {
    /// Revisión leída de la venta antes de mutarla.
    pub expected_sale_version: u64,
    /// Producto de la misma organización que se cotiza en servidor.
    pub product_id: EntityId,
    /// Cantidad entera entre uno y el máximo canónico.
    pub quantity: u32,
    /// Opciones de modificador; nunca contiene precios ni snapshots del cliente.
    pub modifier_option_ids: Vec<EntityId>,
}

/// Entrada para recotizar una partida sin cambiar de producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateSaleItemRequest {
    /// Revisión leída de la venta antes de recotizar la partida.
    pub expected_sale_version: u64,
    /// Nueva cantidad entera de la partida activa.
    pub quantity: u32,
    /// Nueva selección completa de opciones de modificador.
    pub modifier_option_ids: Vec<EntityId>,
}

/// Página determinista de ventas de un turno.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SalePage {
    /// Máximo de filas, entre uno y [`MAX_SALE_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento en el orden histórico determinista.
    pub offset: u32,
}
