use std::fmt;

/// Fallos del catálogo sin datos de entrada ni contenido sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogError {
    /// El código no es un slug ASCII válido o supera el límite.
    InvalidCode,
    /// El SKU no cumple `^[A-Z0-9._-]{1,64}$` tras normalizarse.
    InvalidSku,
    /// El código de barras supera el límite tras el recorte.
    InvalidBarcode,
    /// La descripción supera el límite tras el recorte.
    InvalidDescription,
    /// El nombre no satisface el value object común de nombre visible.
    InvalidName,
    /// El importe no es un decimal exacto admitido por `NUMERIC(19,4)`.
    InvalidPrice,
    /// Los límites de selección del grupo son incoherentes.
    InvalidSelectionRange,
    /// El orden de presentación es negativo.
    InvalidDisplayOrder,
    /// La versión esperada no corresponde al agregado actual.
    ConcurrentModification,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// La lista solicitada repite un grupo de modificadores.
    DuplicateModifierGroup,
    /// La categoría de destino está inactiva.
    CategoryInactive,
    /// La entidad referenciada pertenece a otra organización.
    OwnershipMismatch,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCode => "código de catálogo no válido",
            Self::InvalidSku => "SKU no válido",
            Self::InvalidBarcode => "código de barras no válido",
            Self::InvalidDescription => "descripción no válida",
            Self::InvalidName => "nombre no válido",
            Self::InvalidPrice => "importe no válido",
            Self::InvalidSelectionRange => "límites de selección no válidos",
            Self::InvalidDisplayOrder => "orden de presentación no válido",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::DuplicateModifierGroup => "la lista repite un grupo de modificadores",
            Self::CategoryInactive => "la categoría está inactiva",
            Self::OwnershipMismatch => "la entidad pertenece a otra organización",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for CatalogError {}
