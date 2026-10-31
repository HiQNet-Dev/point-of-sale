//! Dominio puro del catálogo organizacional: categorías, productos y modificadores.
//!
//! Reproduce las reglas del backend Java (`catalog/domain/*`) sin JPA, sin Spring
//! y sin SQLx. El catálogo pertenece a la **organización**; la sucursal solo lo
//! consume en operación. Ninguna entidad se borra físicamente: se retira con
//! [`Status::Inactive`], y esa retirada nunca se propaga a otra fila.
//!
//! # Invariantes
//!
//! - `code` es un slug ASCII normalizado (`java_trim` + minúsculas) e inmutable.
//! - `sku` es el identificador de negocio, normalizado a mayúsculas e inmutable.
//! - `barcode` es opcional, se conserva como texto y distingue `None` de vacío.
//! - Los importes usan escala decimal exacta 4, nunca coma flotante.
//! - Los semánticos de selección de un grupo de modificadores se derivan de
//!   `min_selections`/`max_selections`; jamás se almacenan.
//! - La versión optimista arranca en `0` y solo avanza ante un cambio material.
//!
//! # Organización
//!
//! Por responsabilidad: [`errors`], [`code`] (value objects de texto), [`money`]
//! (decimal exacto), [`patch`], [`category`], [`product`], [`modifier`] y
//! [`ordering`]. La API pública se reexporta aquí.

/// Longitud máxima en unidades UTF-16 de un código de catálogo.
pub const MAX_CATALOG_CODE_UNITS: usize = 64;
/// Longitud máxima en unidades UTF-16 del SKU canónico.
pub const MAX_SKU_UNITS: usize = 64;
/// Longitud máxima en unidades UTF-16 de un código de barras.
pub const MAX_BARCODE_UNITS: usize = 64;
/// Longitud máxima en unidades UTF-16 de una descripción.
pub const MAX_DESCRIPTION_UNITS: usize = 1000;
/// Escala decimal heredada del contrato `NUMERIC(19,4)`.
pub const PRICE_SCALE: u32 = 4;
/// Dígitos enteros máximos heredados del contrato `NUMERIC(19,4)`.
pub const MAX_PRICE_INTEGER_DIGITS: usize = 15;

const MINOR_FACTOR: i128 = 10_000;
const MAX_MINOR_MAGNITUDE: i128 = 10_000 * 1_000_000_000_000_000;

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

mod category;
mod code;
mod errors;
mod modifier;
mod money;
mod ordering;
mod patch;
mod product;

pub use category::{Category, CategoryPatch, NewCategory, category_changed_fields};
pub use code::{Barcode, CatalogCode, Description, Sku};
pub use errors::CatalogError;
pub use modifier::{
    ModifierGroup, ModifierGroupPatch, ModifierOption, ModifierOptionPatch, ModifierReplacement,
    NewModifierGroup, NewModifierOption, modifier_group_changed_fields,
    modifier_option_changed_fields, plan_modifier_replacement,
};
pub use money::{Decimal4, PriceDelta, UnitPrice};
pub use ordering::display_order;
pub use patch::PatchField;
pub use product::{NewProduct, Product, ProductPatch, product_changed_fields};
