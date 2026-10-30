use super::*;
use crate::organization_branch::java_trim;

/// Código de catálogo: slug ASCII minúsculo e inmutable.
///
/// Normaliza con `java_trim` y minúsculas ASCII, y exige
/// `^[a-z0-9]+(-[a-z0-9]+)*$` con hasta [`MAX_CATALOG_CODE_UNITS`] unidades
/// UTF-16. Una entrada Unicode se normaliza igual que en Java y termina
/// rechazada por la gramática, que solo admite ASCII.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CatalogCode(String);

impl CatalogCode {
    /// Normaliza y valida el código.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidCode`] si el resultado no es un slug.
    pub fn new(value: &str) -> Result<Self, CatalogError> {
        let normalized = java_trim(value).to_ascii_lowercase();
        let bytes = normalized.as_bytes();
        let valid = !bytes.is_empty()
            && utf16_len(&normalized) <= MAX_CATALOG_CODE_UNITS
            && bytes
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
            && !normalized.starts_with('-')
            && !normalized.ends_with('-')
            && !normalized.contains("--");
        if !valid {
            return Err(CatalogError::InvalidCode);
        }
        Ok(Self(normalized))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        let parsed = Self::new(value).map_err(|_| CatalogError::InvalidPersistedValue)?;
        if parsed.as_str() != value {
            return Err(CatalogError::InvalidPersistedValue);
        }
        Ok(parsed)
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// SKU canónico: identificador de negocio inmutable de un producto.
///
/// Normaliza con `java_trim` y mayúsculas Unicode, y exige
/// `^[A-Z0-9._-]{1,64}$` con hasta [`MAX_SKU_UNITS`] unidades UTF-16.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Sku(String);

impl Sku {
    /// Normaliza y valida el SKU.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidSku`] si el resultado no cumple la gramática.
    pub fn new(value: &str) -> Result<Self, CatalogError> {
        let normalized = java_trim(value).to_uppercase();
        let bytes = normalized.as_bytes();
        let valid = !bytes.is_empty()
            && utf16_len(&normalized) <= MAX_SKU_UNITS
            && bytes.iter().all(|byte| {
                byte.is_ascii_uppercase()
                    || byte.is_ascii_digit()
                    || matches!(*byte, b'.' | b'_' | b'-')
            });
        if !valid {
            return Err(CatalogError::InvalidSku);
        }
        Ok(Self(normalized))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        let parsed = Self::new(value).map_err(|_| CatalogError::InvalidPersistedValue)?;
        if parsed.as_str() != value {
            return Err(CatalogError::InvalidPersistedValue);
        }
        Ok(parsed)
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Código de barras opcional conservado como texto.
///
/// Un valor en blanco se normaliza a `None`; nunca se recorta por dentro ni se
/// rellena, para preservar ceros iniciales.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Barcode(String);

impl Barcode {
    /// Normaliza el código de barras, devolviendo `None` si queda vacío.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidBarcode`] si supera el límite.
    pub fn new(value: &str) -> Result<Option<Self>, CatalogError> {
        let trimmed = java_trim(value);
        if trimmed.is_empty() {
            return Ok(None);
        }
        if utf16_len(trimmed) > MAX_BARCODE_UNITS {
            return Err(CatalogError::InvalidBarcode);
        }
        Ok(Some(Self(trimmed.to_owned())))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        match Self::new(value) {
            Ok(Some(parsed)) if parsed.as_str() == value => Ok(parsed),
            _ => Err(CatalogError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Descripción opcional de producto.
///
/// Un valor en blanco se normaliza a `None` para que "sin descripción" tenga una
/// única representación, igual que el agregado Java.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Description(String);

impl Description {
    /// Normaliza la descripción, devolviendo `None` si queda vacía.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidDescription`] si supera el límite.
    pub fn new(value: &str) -> Result<Option<Self>, CatalogError> {
        let trimmed = java_trim(value);
        if trimmed.is_empty() {
            return Ok(None);
        }
        if utf16_len(trimmed) > MAX_DESCRIPTION_UNITS {
            return Err(CatalogError::InvalidDescription);
        }
        Ok(Some(Self(trimmed.to_owned())))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        match Self::new(value) {
            Ok(Some(parsed)) if parsed.as_str() == value => Ok(parsed),
            _ => Err(CatalogError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
