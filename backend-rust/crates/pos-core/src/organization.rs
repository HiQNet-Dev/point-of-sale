//! Agregado puro de configuración de organización.
//!
//! La unicidad del código, la generación de UUID, el reloj, el control CAS en
//! almacenamiento, auditoría y autorización pertenecen a capas externas.

use std::{fmt, str::FromStr};

use chrono::FixedOffset;
use chrono_tz::Tz;
use icu_locale_core::Locale;
use iso_currency::Currency as IsoCurrency;
use uuid::Uuid;

/// Error de una invariante de Organization o Branch, sin incluir entradas crudas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainError {
    /// El nombre es vacío después de trim o excede su límite.
    InvalidName,
    /// El código no es un slug canónico válido.
    InvalidCode,
    /// La zona no pertenece al catálogo IANA embebido.
    InvalidTimezone,
    /// La moneda no pertenece al catálogo ISO 4217.
    InvalidCurrency,
    /// El locale no es un tag BCP-47 canónico.
    InvalidLocale,
    /// La revisión esperada no coincide con el estado del agregado.
    VersionMismatch,
    /// Incrementar una revisión excedería el rango representable.
    VersionOverflow,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidName => "nombre inválido",
            Self::InvalidCode => "código inválido",
            Self::InvalidTimezone => "zona horaria IANA inválida",
            Self::InvalidCurrency => "moneda ISO 4217 inválida",
            Self::InvalidLocale => "locale BCP-47 inválido",
            Self::VersionMismatch => "la revisión esperada no coincide",
            Self::VersionOverflow => "la revisión excede el rango representable",
        })
    }
}

impl std::error::Error for DomainError {}

/// Identidad estable de una organización; el llamador la genera y conserva.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrganizationId(Uuid);

impl OrganizationId {
    /// Construye la identidad desde un UUID ya asignado fuera del dominio.
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Devuelve el UUID estable de la organización.
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Identidad estable de una sucursal; no implica su organización propietaria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BranchId(Uuid);

impl BranchId {
    /// Construye la identidad desde un UUID ya asignado fuera del dominio.
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Devuelve el UUID estable de la sucursal.
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Nombre visible normalizado al eliminar espacios extremos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayName(String);

impl DisplayName {
    /// Valida y normaliza un nombre de hasta 120 caracteres Unicode.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let normalized = value.trim();
        if normalized.is_empty() || normalized.chars().count() > 120 {
            return Err(DomainError::InvalidName);
        }
        Ok(Self(normalized.into()))
    }

    /// Devuelve el nombre persistible sin espacios extremos.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Código de negocio canónico: slug ASCII minúsculo de hasta 64 caracteres.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BusinessCode(String);

impl BusinessCode {
    /// Rechaza formas no canónicas; la normalización de adaptadores es explícita.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let valid = !value.is_empty()
            && value.len() <= 64
            && value.split('-').all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            });
        if !valid {
            return Err(DomainError::InvalidCode);
        }
        Ok(Self(value.into()))
    }

    /// Devuelve el slug canónico e inmutable.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// ZoneId Java validada: zona TZDB nombrada u offset fijo, no un string libre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IanaTimezone {
    value: String,
}

impl IanaTimezone {
    /// Valida un `ZoneId` Java no vacío de hasta 64 bytes UTF-8.
    ///
    /// Acepta zonas TZDB nombradas y offsets fijos (`+01:00`, `UTC+01:00`,
    /// `GMT+01:00` o `UT+01:00`) que `ZoneId.of` también admite.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let named_zone = Tz::from_str(value).is_ok();
        let fixed_offset = FixedOffset::from_str(value).is_ok()
            || ["UTC", "GMT", "UT"].iter().any(|prefix| {
                value
                    .strip_prefix(prefix)
                    .is_some_and(|offset| FixedOffset::from_str(offset).is_ok())
            });
        if value.is_empty() || value.len() > 64 || (!named_zone && !fixed_offset) {
            return Err(DomainError::InvalidTimezone);
        }
        Ok(Self {
            value: value.into(),
        })
    }

    /// Devuelve la zona validada, preservando su representación de entrada Java.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Código ISO 4217 validado y normalizado a mayúsculas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationCurrency(String);

impl OrganizationCurrency {
    /// Valida un código ISO 4217 de tres letras, sin conocer minor digits.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(DomainError::InvalidCurrency);
        }
        let normalized = value.to_ascii_uppercase();
        if IsoCurrency::from_code(&normalized).is_none() {
            return Err(DomainError::InvalidCurrency);
        }
        Ok(Self(normalized))
    }

    /// Devuelve el código ISO 4217 en mayúsculas.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Tag BCP-47 validado; preserva la forma aceptada por Java para persistencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleTag(String);

impl LocaleTag {
    /// Valida un locale BCP-47 de hasta 20 bytes y su representación canónica.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        if value.is_empty() || value.len() > 20 {
            return Err(DomainError::InvalidLocale);
        }
        let parsed = Locale::try_from_str(value).map_err(|_| DomainError::InvalidLocale)?;
        if !parsed.to_string().eq_ignore_ascii_case(value) {
            return Err(DomainError::InvalidLocale);
        }
        Ok(Self(value.into()))
    }

    /// Devuelve el tag persistible validado.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Estado operacional de la organización.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizationStatus {
    /// La organización puede operar, sujeto a autorización externa.
    Active,
    /// La organización conserva hechos pero no está operativa.
    Inactive,
}

/// Política legada de stock negativo por organización.
///
/// Solo conserva los valores AS-IS. No representa pending, retornos, valoración
/// ni decisiones de costo sujetas a CC-0021.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryNegativeStockPolicy {
    /// Un checkout futuro debe rechazar insuficiencia de stock rastreado.
    Strict,
    /// Un checkout futuro puede permitir un saldo físico negativo.
    AllowNegative,
}

/// Revisión optimista representable en el dominio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision(u64);

impl Revision {
    /// Revisión inicial de un agregado recién creado.
    pub const INITIAL: Self = Self(0);

    /// Mayor revisión compatible con `BIGINT` positivo de Java/PostgreSQL.
    pub const MAX_VALUE: u64 = i64::MAX as u64;

    /// Construye una revisión recuperada de persistencia compatible con Java.
    pub fn try_new(value: u64) -> Result<Self, DomainError> {
        if value > Self::MAX_VALUE {
            return Err(DomainError::VersionOverflow);
        }
        Ok(Self(value))
    }

    /// Devuelve el valor para una frontera de aplicación o almacenamiento.
    pub const fn value(self) -> u64 {
        self.0
    }

    fn next(self) -> Result<Self, DomainError> {
        Self::try_new(self.0.checked_add(1).ok_or(DomainError::VersionOverflow)?)
    }
}

/// Campos de Organization que pueden actualizarse sin cambiar su código o ID.
#[derive(Debug, Default)]
pub struct OrganizationPatch {
    /// Nuevo nombre; `None` significa no modificar.
    pub name: Option<DisplayName>,
    /// Nueva zona horaria; `None` significa no modificar.
    pub timezone: Option<IanaTimezone>,
    /// Nueva moneda; `None` significa no modificar.
    pub currency: Option<OrganizationCurrency>,
    /// Nuevo locale; `None` significa no modificar.
    pub locale: Option<LocaleTag>,
    /// Nuevo estado; `None` significa no modificar.
    pub status: Option<OrganizationStatus>,
    /// Nueva política legada; `None` significa no modificar.
    pub inventory_negative_stock_policy: Option<InventoryNegativeStockPolicy>,
}

/// Resultado observable de un patch puro; auditoría/persistencia lo consumen fuera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFields(pub(crate) Vec<&'static str>);

impl ChangedFields {
    /// No hubo modificación material.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Campos modificados en el orden estable observado en Java.
    pub fn as_slice(&self) -> &[&'static str] {
        &self.0
    }
}

/// Agregado de organización sin transporte, persistencia, reloj ni autorización.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    id: OrganizationId,
    name: DisplayName,
    code: BusinessCode,
    timezone: IanaTimezone,
    currency: OrganizationCurrency,
    locale: LocaleTag,
    status: OrganizationStatus,
    inventory_negative_stock_policy: InventoryNegativeStockPolicy,
    revision: Revision,
}

impl Organization {
    /// Crea una organización válida con defaults AS-IS `ACTIVE` y `STRICT`.
    pub fn create(
        id: OrganizationId,
        name: DisplayName,
        code: BusinessCode,
        timezone: IanaTimezone,
        currency: OrganizationCurrency,
        locale: LocaleTag,
    ) -> Self {
        Self {
            id,
            name,
            code,
            timezone,
            currency,
            locale,
            status: OrganizationStatus::Active,
            inventory_negative_stock_policy: InventoryNegativeStockPolicy::Strict,
            revision: Revision::INITIAL,
        }
    }

    /// Aplica un patch si la revisión esperada coincide y devuelve cambios reales.
    pub fn apply_patch(
        &mut self,
        expected_revision: Revision,
        patch: OrganizationPatch,
    ) -> Result<ChangedFields, DomainError> {
        if self.revision != expected_revision {
            return Err(DomainError::VersionMismatch);
        }
        let mut candidate = self.clone();
        let mut changed = Vec::new();
        apply_if_changed(&mut candidate.name, patch.name, "name", &mut changed);
        apply_if_changed(
            &mut candidate.timezone,
            patch.timezone,
            "timezone",
            &mut changed,
        );
        apply_if_changed(
            &mut candidate.currency,
            patch.currency,
            "currency",
            &mut changed,
        );
        apply_if_changed(&mut candidate.locale, patch.locale, "locale", &mut changed);
        apply_if_changed(&mut candidate.status, patch.status, "status", &mut changed);
        apply_if_changed(
            &mut candidate.inventory_negative_stock_policy,
            patch.inventory_negative_stock_policy,
            "inventoryNegativeStockPolicy",
            &mut changed,
        );
        if !changed.is_empty() {
            candidate.revision = candidate.revision.next()?;
            *self = candidate;
        }
        Ok(ChangedFields(changed))
    }

    /// Identidad estable e inmutable.
    pub const fn id(&self) -> OrganizationId {
        self.id
    }
    /// Nombre visible validado.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }
    /// Código de negocio inmutable.
    pub fn code(&self) -> &BusinessCode {
        &self.code
    }
    /// Zona horaria propia obligatoria.
    pub fn timezone(&self) -> &IanaTimezone {
        &self.timezone
    }
    /// Moneda ISO 4217 predeterminada.
    pub fn currency(&self) -> &OrganizationCurrency {
        &self.currency
    }
    /// Locale BCP-47 configurado.
    pub fn locale(&self) -> &LocaleTag {
        &self.locale
    }
    /// Estado operacional.
    pub const fn status(&self) -> OrganizationStatus {
        self.status
    }
    /// Política legada de stock negativo.
    pub const fn inventory_negative_stock_policy(&self) -> InventoryNegativeStockPolicy {
        self.inventory_negative_stock_policy
    }
    /// Revisión optimista del estado en memoria.
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    /// Indica si el estado es operativo; autorización sigue siendo externa.
    pub const fn is_active(&self) -> bool {
        matches!(self.status, OrganizationStatus::Active)
    }

    #[cfg(test)]
    pub(crate) fn set_revision_for_test(&mut self, revision: Revision) {
        self.revision = revision;
    }
}

fn apply_if_changed<T: PartialEq>(
    current: &mut T,
    replacement: Option<T>,
    name: &'static str,
    changed: &mut Vec<&'static str>,
) {
    if let Some(replacement) = replacement
        && *current != replacement
    {
        *current = replacement;
        changed.push(name);
    }
}
