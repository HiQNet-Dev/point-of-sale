use super::*;

/// Estado operativo de una organización o sucursal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// Recurso disponible para operación normal.
    Active,
    /// Recurso conservado históricamente, pero no operativo.
    Inactive,
}

impl Status {
    /// Valor almacenado en el esquema SQLite experimental.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Inactive => "INACTIVE",
        }
    }

    /// Convierte el valor almacenado sin aceptar estados desconocidos.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "INACTIVE" => Ok(Self::Inactive),
            _ => Err(DomainError::InvalidPersistedValue),
        }
    }
}

/// Política heredada de stock negativo de la organización.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NegativeStockPolicy {
    /// No se permite confirmar stock negativo donde la política sea aplicable.
    Strict,
    /// La operación que consulte esta política puede permitir saldo negativo.
    AllowNegative,
}

impl NegativeStockPolicy {
    /// Valor persistido compatible con el DDL local.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "STRICT",
            Self::AllowNegative => "ALLOW_NEGATIVE",
        }
    }

    /// Lee una política persistida conocida.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "STRICT" => Ok(Self::Strict),
            "ALLOW_NEGATIVE" => Ok(Self::AllowNegative),
            _ => Err(DomainError::InvalidPersistedValue),
        }
    }
}

/// Zona horaria compatible con las formas Java `ZoneId` necesarias para Local.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timezone(String);

impl Timezone {
    /// Acepta lo que `ZoneId.of` de Java admite para Local: aliases UTC/GMT/UT,
    /// `Z`, offsets fijos de hasta ±18:00 y zonas presentes en TZDB.
    ///
    /// Las zonas nombradas se contrastan contra la base TZDB incluida; una forma
    /// textual plausible pero inexistente se rechaza.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        if value.is_empty() || value.chars().any(char::is_whitespace) || utf16_len(value) > 64 {
            return Err(DomainError::InvalidTimezone);
        }
        let valid = matches!(value, "Z" | "UTC" | "GMT" | "UT")
            || valid_offset(value)
            || valid_prefixed_offset(value)
            || valid_tzdb_zone(value);
        if !valid {
            return Err(DomainError::InvalidTimezone);
        }
        Ok(Self(value.to_owned()))
    }

    /// Valor textual conservado sin una normalización que Java no realiza.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Código ISO 4217 reconocido por el JDK de referencia del repositorio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsoCurrency(String);

impl IsoCurrency {
    /// Valida y normaliza mayúsculas ASCII sin aceptar códigos sintácticos desconocidos.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(DomainError::InvalidCurrency);
        }
        let normalized = value.to_ascii_uppercase();
        if !JAVA_CURRENCIES.binary_search(&normalized.as_str()).is_ok() {
            return Err(DomainError::InvalidCurrency);
        }
        Ok(Self(normalized))
    }

    /// Código ISO persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Locale BCP-47 validado con el perfil de tags bien formados que necesita el dominio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocaleTag(String);

impl LocaleTag {
    /// Rechaza tags vacíos, mal formados, no canónicos o mayores de 20 unidades UTF-16.
    ///
    /// El perfil exige la misma forma que `Locale.toLanguageTag()` de Java, de modo
    /// que una variante como `es_mx` o `ES-mx` no se persiste como si fuera otra.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        if value.is_empty() || utf16_len(value) > 20 || value.contains('_') {
            return Err(DomainError::InvalidLocale);
        }
        let mut parts = value.split('-');
        let Some(language) = parts.next() else {
            return Err(DomainError::InvalidLocale);
        };
        if !(2..=8).contains(&language.len())
            || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            return Err(DomainError::InvalidLocale);
        }
        if !parts.all(|part| {
            (1..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }) {
            return Err(DomainError::InvalidLocale);
        }
        let canonical =
            icu_locale_core::Locale::try_from_str(value).map_err(|_| DomainError::InvalidLocale)?;
        if !canonical.to_string().eq_ignore_ascii_case(value) {
            return Err(DomainError::InvalidLocale);
        }
        Ok(Self(value.to_owned()))
    }

    /// Tag original; Java conserva la semántica de case-insensitive roundtrip.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
