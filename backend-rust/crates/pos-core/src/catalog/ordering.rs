use super::*;

/// Convierte un orden de presentación de transporte a `u32`.
///
/// # Errors
/// Devuelve [`CatalogError::InvalidDisplayOrder`] si el valor es negativo.
pub fn display_order(value: i64) -> Result<u32, CatalogError> {
    u32::try_from(value).map_err(|_| CatalogError::InvalidDisplayOrder)
}
