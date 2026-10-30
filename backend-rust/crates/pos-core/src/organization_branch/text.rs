use super::*;

/// Recorte con la semántica de `String.trim()` de Java: U+0000..U+0020.
pub fn java_trim(value: &str) -> &str {
    value.trim_matches(|character: char| character <= '\u{20}')
}

/// Nombre de usuario preservando espacios internos y normalizando extremos Java.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayName(String);

impl DisplayName {
    /// Construye un nombre no vacío de hasta 120 unidades UTF-16.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        let normalized = java_trim(value);
        if normalized.is_empty() || utf16_len(normalized) > 120 {
            return Err(DomainError::InvalidName);
        }
        Ok(Self(normalized.to_owned()))
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Código inmutable normalizado como Java: trim y lowercase ASCII de `Locale.ROOT`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BusinessCode(String);

impl BusinessCode {
    /// Normaliza y valida el slug ASCII de hasta 64 unidades UTF-16.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        let normalized = java_trim(value).to_ascii_lowercase();
        let bytes = normalized.as_bytes();
        let valid = !bytes.is_empty()
            && utf16_len(&normalized) <= 64
            && bytes.iter().all(u8::is_ascii_lowercase_or_digit_or_dash)
            && !normalized.starts_with('-')
            && !normalized.ends_with('-')
            && !normalized.contains("--");
        if !valid {
            return Err(DomainError::InvalidCode);
        }
        Ok(Self(normalized))
    }

    /// Código normalizado persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

trait AsciiSlugByte {
    fn is_ascii_lowercase_or_digit_or_dash(&self) -> bool;
}

impl AsciiSlugByte for u8 {
    fn is_ascii_lowercase_or_digit_or_dash(&self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit() || *self == b'-'
    }
}
