//! DTOs de aplicación del catálogo.
//!
//! No contienen SQL; el adaptador los consume.

use pos_core::{
    catalog::PatchField,
    organization_branch::{EntityId, Status},
};

/// Datos de alta de categoría recibidos por el caso de uso.
///
/// Llegan sin validar: el caso de uso aplica las mismas reglas que Java
/// (`java_trim`, slug en minúsculas, nombre visible).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCategoryRequest {
    /// Código de negocio propuesto.
    pub code: String,
    /// Nombre visible propuesto.
    pub name: String,
}

/// Datos de alta de producto recibidos por el caso de uso.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewProductRequest {
    /// SKU propuesto; se normaliza a mayúsculas canónicas.
    pub sku: String,
    /// Nombre visible propuesto.
    pub name: String,
    /// Precio base en texto decimal, escala máxima 4.
    pub unit_price: String,
    /// Código de barras opcional.
    pub barcode: Option<String>,
    /// Descripción opcional.
    pub description: Option<String>,
    /// Categoría destino de la misma organización.
    pub category_id: Option<EntityId>,
}

/// Cambios de categoría. El código y la organización nunca se modifican.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryUpdateRequest {
    /// Versión leída por el llamador.
    pub expected_version: u64,
    /// Nombre nuevo.
    pub name: Option<String>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Cambios de producto con distinción de ausencia y limpieza.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductUpdateRequest {
    /// Versión leída por el llamador.
    pub expected_version: u64,
    /// Nombre nuevo, ya recortado o no.
    pub name: Option<String>,
    /// Descripción: ausente, limpiada o nueva.
    pub description: PatchField<String>,
    /// Código de barras: ausente, limpiado o nuevo.
    pub barcode: PatchField<String>,
    /// Precio base nuevo.
    pub unit_price: Option<String>,
    /// Categoría: ausente, limpiada o referenciada por identidad.
    pub category_id: PatchField<EntityId>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Ventana de lectura del catálogo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogPage {
    /// Tamaño de página, de 1 a [`MAX_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento desde el inicio del orden por nombre.
    pub offset: u32,
}

/// Tamaño máximo de página para listados de catálogo.
pub const MAX_PAGE_SIZE: u32 = 100;
