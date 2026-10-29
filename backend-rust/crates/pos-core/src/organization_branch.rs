//! Dominio y puertos de aplicación de organizaciones y sucursales.
//!
//! Este módulo se reconstruyó tras `INC-RECOVERY-01`: el incremento histórico
//! 1A no era recuperable en el entorno actual. No depende de SQLx, HTTP ni de
//! ningún framework de transporte.

use std::fmt;

/// Identificador UUID canónico y estable asignado por la aplicación.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(String);

impl EntityId {
    /// Valida un UUID textual canónico en minúsculas.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let bytes = value.as_bytes();
        if bytes.len() != 36
            || ![8, 13, 18, 23]
                .into_iter()
                .all(|index| bytes[index] == b'-')
            || !bytes.iter().enumerate().all(|(index, byte)| {
                [8, 13, 18, 23].contains(&index)
                    || byte.is_ascii_digit()
                    || (b'a'..=b'f').contains(byte)
            })
        {
            return Err(DomainError::InvalidUuid);
        }
        Ok(Self(value.to_owned()))
    }

    /// Devuelve la representación canónica persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

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

/// Errores puros del dominio; no representan estados HTTP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainError {
    /// El UUID no tiene formato canónico minúsculo.
    InvalidUuid,
    /// Un nombre es vacío tras el recorte Java o supera su límite UTF-16.
    InvalidName,
    /// Un código de negocio no es un slug ASCII válido.
    InvalidCode,
    /// La zona no representa un identificador compatible conocido.
    InvalidTimezone,
    /// El código no pertenece al catálogo `Currency` del JDK de referencia.
    InvalidCurrency,
    /// El tag BCP-47 no cumple el perfil compatible implementado.
    InvalidLocale,
    /// La revisión recibida ya no corresponde al agregado.
    StaleRevision,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// El owner inmutable de una sucursal no coincide con el solicitado.
    OwnershipMismatch,
    /// Una fila persistida incumple invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUuid => "UUID no canónico",
            Self::InvalidName => "nombre no válido",
            Self::InvalidCode => "código no válido",
            Self::InvalidTimezone => "zona horaria no válida",
            Self::InvalidCurrency => "moneda ISO 4217 no válida",
            Self::InvalidLocale => "locale BCP-47 no válido",
            Self::StaleRevision => "revisión obsoleta",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::OwnershipMismatch => "la sucursal no pertenece a la organización",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for DomainError {}

/// Recorte con la semántica de `String.trim()` de Java: U+0000..U+0020.
pub fn java_trim(value: &str) -> &str {
    value.trim_matches(|character: char| character <= '\u{20}')
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
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

/// Zona horaria compatible con las formas Java `ZoneId` necesarias para Local.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timezone(String);

impl Timezone {
    /// Acepta aliases Java UTC/GMT/UT, `Z`, offsets hasta ±18:00 e IDs TZDB estructurados.
    ///
    /// La validación de reglas históricas TZDB queda delegada al runtime de
    /// presentación/infraestructura hasta incorporar una base TZDB versionada.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        if value.is_empty() || value.chars().any(char::is_whitespace) || utf16_len(value) > 64 {
            return Err(DomainError::InvalidTimezone);
        }
        let valid = matches!(value, "Z" | "UTC" | "GMT" | "UT")
            || valid_offset(value)
            || valid_prefixed_offset(value)
            || valid_tzdb_shape(value);
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

fn valid_prefixed_offset(value: &str) -> bool {
    for prefix in ["UTC", "GMT", "UT"] {
        if let Some(offset) = value.strip_prefix(prefix) {
            return valid_offset(offset);
        }
    }
    false
}

fn valid_offset(value: &str) -> bool {
    let Some((sign, rest)) = value.as_bytes().split_first() else {
        return false;
    };
    if !matches!(sign, b'+' | b'-') {
        return false;
    }
    let (hour, minute) = match rest {
        [a, b] => (Some([*a, *b]), Some(*b"00")),
        [a, b, b':', c, d] => (Some([*a, *b]), Some([*c, *d])),
        [a, b, c, d] => (Some([*a, *b]), Some([*c, *d])),
        _ => (None, None),
    };
    let (Some(hour), Some(minute)) = (hour, minute) else {
        return false;
    };
    if !hour
        .into_iter()
        .chain(minute)
        .all(|digit| digit.is_ascii_digit())
    {
        return false;
    }
    let hours = u16::from(hour[0] - b'0') * 10 + u16::from(hour[1] - b'0');
    let minutes = u16::from(minute[0] - b'0') * 10 + u16::from(minute[1] - b'0');
    hours < 18 || (hours == 18 && minutes == 0) || (hours < 18 && minutes < 60)
}

fn valid_tzdb_shape(value: &str) -> bool {
    value.contains('/')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'.')
                })
        })
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

const JAVA_CURRENCIES: &[&str] = &[
    "ADP", "AED", "AFA", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "ATS", "AUD", "AWG", "AYM",
    "AZM", "AZN", "BAM", "BBD", "BDT", "BEF", "BGL", "BGN", "BHD", "BIF", "BMD", "BND", "BOB",
    "BOV", "BRL", "BSD", "BTN", "BWP", "BYB", "BYN", "BYR", "BZD", "CAD", "CDF", "CHE", "CHF",
    "CHW", "CLF", "CLP", "CNY", "COP", "COU", "CRC", "CSD", "CUC", "CUP", "CVE", "CYP", "CZK",
    "DEM", "DJF", "DKK", "DOP", "DZD", "EEK", "EGP", "ERN", "ESP", "ETB", "EUR", "FIM", "FJD",
    "FKP", "FRF", "GBP", "GEL", "GHC", "GHS", "GIP", "GMD", "GNF", "GRD", "GTQ", "GWP", "GYD",
    "HKD", "HNL", "HRK", "HTG", "HUF", "IDR", "IEP", "ILS", "INR", "IQD", "IRR", "ISK", "ITL",
    "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF", "KPW", "KRW", "KWD", "KYD", "KZT", "LAK",
    "LBP", "LKR", "LRD", "LSL", "LTL", "LUF", "LVL", "LYD", "MAD", "MDL", "MGA", "MGF", "MKD",
    "MMK", "MNT", "MOP", "MRO", "MRU", "MTL", "MUR", "MVR", "MWK", "MXN", "MXV", "MYR", "MZM",
    "MZN", "NAD", "NGN", "NIO", "NLG", "NOK", "NPR", "NZD", "OMR", "PAB", "PEN", "PGK", "PHP",
    "PKR", "PLN", "PTE", "PYG", "QAR", "ROL", "RON", "RSD", "RUB", "RUR", "RWF", "SAR", "SBD",
    "SCR", "SDD", "SDG", "SEK", "SGD", "SHP", "SIT", "SKK", "SLE", "SLL", "SOS", "SRD", "SRG",
    "SSP", "STD", "STN", "SVC", "SYP", "SZL", "THB", "TJS", "TMM", "TMT", "TND", "TOP", "TPE",
    "TRL", "TRY", "TTD", "TWD", "TZS", "UAH", "UGX", "USD", "USN", "USS", "UYI", "UYU", "UZS",
    "VEB", "VED", "VEF", "VES", "VND", "VUV", "WST", "XAD", "XAF", "XAG", "XAU", "XBA", "XBB",
    "XBC", "XBD", "XCD", "XCG", "XDR", "XFO", "XFU", "XOF", "XPD", "XPF", "XPT", "XSU", "XTS",
    "XUA", "XXX", "YER", "YUM", "ZAR", "ZMK", "ZMW", "ZWD", "ZWG", "ZWL", "ZWN", "ZWR",
];

/// Locale BCP-47 validado con el perfil de tags bien formados que necesita el dominio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocaleTag(String);

impl LocaleTag {
    /// Rechaza tags vacíos, subtags mal formados o mayores de 20 unidades UTF-16.
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
        Ok(Self(value.to_owned()))
    }

    /// Tag original; Java conserva la semántica de case-insensitive roundtrip.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Datos necesarios para crear una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewOrganization {
    /// Nombre visible.
    pub name: DisplayName,
    /// Código global inmutable.
    pub code: BusinessCode,
    /// Zona horaria raíz.
    pub timezone: Timezone,
    /// Moneda por defecto.
    pub currency: IsoCurrency,
    /// Locale por defecto.
    pub locale: LocaleTag,
}

/// Agregado raíz de configuración de una instalación.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Organization {
    id: EntityId,
    name: DisplayName,
    code: BusinessCode,
    timezone: Timezone,
    currency: IsoCurrency,
    locale: LocaleTag,
    status: Status,
    negative_stock_policy: NegativeStockPolicy,
    revision: u64,
}

impl Organization {
    /// Crea un agregado nuevo con defaults compatibles con Java.
    pub fn create(id: EntityId, input: NewOrganization) -> Self {
        Self {
            id,
            name: input.name,
            code: input.code,
            timezone: input.timezone,
            currency: input.currency,
            locale: input.locale,
            status: Status::Active,
            negative_stock_policy: NegativeStockPolicy::Strict,
            revision: 0,
        }
    }

    /// Reconstruye una fila previamente validada por este módulo.
    #[allow(clippy::too_many_arguments)]
    pub fn rehydrate(
        id: EntityId,
        name: DisplayName,
        code: BusinessCode,
        timezone: Timezone,
        currency: IsoCurrency,
        locale: LocaleTag,
        status: Status,
        negative_stock_policy: NegativeStockPolicy,
        revision: u64,
    ) -> Self {
        Self {
            id,
            name,
            code,
            timezone,
            currency,
            locale,
            status,
            negative_stock_policy,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Código inmutable.
    pub fn code(&self) -> &BusinessCode {
        &self.code
    }
    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }
    /// Zona horaria raíz.
    pub fn timezone(&self) -> &Timezone {
        &self.timezone
    }
    /// Moneda de la organización.
    pub fn currency(&self) -> &IsoCurrency {
        &self.currency
    }
    /// Locale de la organización.
    pub fn locale(&self) -> &LocaleTag {
        &self.locale
    }
    /// Estado operacional.
    pub fn status(&self) -> Status {
        self.status
    }
    /// Política negativa heredada.
    pub fn negative_stock_policy(&self) -> NegativeStockPolicy {
        self.negative_stock_policy
    }
    /// Revisión optimista actual.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Aplica un patch de forma atómica; un error deja el agregado intacto.
    pub fn patch(&mut self, patch: OrganizationPatch) -> Result<bool, DomainError> {
        if self.revision != patch.expected_revision {
            return Err(DomainError::StaleRevision);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        if let Some(value) = patch.timezone {
            changed |= next.timezone != value;
            next.timezone = value;
        }
        if let Some(value) = patch.currency {
            changed |= next.currency != value;
            next.currency = value;
        }
        if let Some(value) = patch.locale {
            changed |= next.locale != value;
            next.locale = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if let Some(value) = patch.negative_stock_policy {
            changed |= next.negative_stock_policy != value;
            next.negative_stock_policy = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(DomainError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch parcial; código e identidad no están presentes por diseño.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrganizationPatch {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nuevo nombre opcional.
    pub name: Option<DisplayName>,
    /// Nueva zona horaria opcional.
    pub timezone: Option<Timezone>,
    /// Nueva moneda opcional.
    pub currency: Option<IsoCurrency>,
    /// Nuevo locale opcional.
    pub locale: Option<LocaleTag>,
    /// Nuevo estado opcional.
    pub status: Option<Status>,
    /// Nueva política de stock opcional.
    pub negative_stock_policy: Option<NegativeStockPolicy>,
}

/// Valor triestado del override de zona de una sucursal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimezoneOverridePatch {
    /// El campo fue omitido y no cambia.
    Keep,
    /// Define un override explícito.
    Set(Timezone),
    /// Elimina el override y hereda de la organización.
    Clear,
}

/// Datos de creación de una sucursal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewBranch {
    /// Nombre visible.
    pub name: DisplayName,
    /// Código único dentro del owner.
    pub code: BusinessCode,
    /// Override opcional; `None` hereda.
    pub timezone_override: Option<Timezone>,
}

/// Agregado de sucursal, siempre propiedad de una única organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    id: EntityId,
    organization_id: EntityId,
    name: DisplayName,
    code: BusinessCode,
    timezone_override: Option<Timezone>,
    status: Status,
    revision: u64,
}

impl Branch {
    /// Crea una sucursal activa con revisión cero.
    pub fn create(id: EntityId, organization_id: EntityId, input: NewBranch) -> Self {
        Self {
            id,
            organization_id,
            name: input.name,
            code: input.code,
            timezone_override: input.timezone_override,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Reconstruye una fila validada.
    #[allow(clippy::too_many_arguments)]
    pub fn rehydrate(
        id: EntityId,
        organization_id: EntityId,
        name: DisplayName,
        code: BusinessCode,
        timezone_override: Option<Timezone>,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            name,
            code,
            timezone_override,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Owner inmutable.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Código inmutable dentro del owner.
    pub fn code(&self) -> &BusinessCode {
        &self.code
    }
    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }
    /// Override opcional.
    pub fn timezone_override(&self) -> Option<&Timezone> {
        self.timezone_override.as_ref()
    }
    /// Resuelve el override o la zona de la organización recibida.
    pub fn effective_timezone<'a>(&'a self, organization_timezone: &'a Timezone) -> &'a Timezone {
        self.timezone_override
            .as_ref()
            .unwrap_or(organization_timezone)
    }
    /// Estado.
    pub fn status(&self) -> Status {
        self.status
    }
    /// Revisión.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Aplica un patch atómicamente y comprueba explícitamente el owner de contexto.
    pub fn patch(
        &mut self,
        organization_id: &EntityId,
        patch: BranchPatch,
    ) -> Result<bool, DomainError> {
        if &self.organization_id != organization_id {
            return Err(DomainError::OwnershipMismatch);
        }
        if self.revision != patch.expected_revision {
            return Err(DomainError::StaleRevision);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        match patch.timezone_override {
            TimezoneOverridePatch::Keep => {}
            TimezoneOverridePatch::Set(value) => {
                changed |= next.timezone_override.as_ref() != Some(&value);
                next.timezone_override = Some(value);
            }
            TimezoneOverridePatch::Clear => {
                changed |= next.timezone_override.is_some();
                next.timezone_override = None;
            }
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(DomainError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch de sucursal: ni identidad, owner ni código pueden modificarse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPatch {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nombre opcional.
    pub name: Option<DisplayName>,
    /// Operación triestado sobre el override.
    pub timezone_override: TimezoneOverridePatch,
    /// Estado opcional.
    pub status: Option<Status>,
}

/// Evento de auditoría aún independiente de su representación persistida.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEvent {
    /// Organización a la que pertenece el evento.
    pub organization_id: EntityId,
    /// Sucursal opcional afectada.
    pub branch_id: Option<EntityId>,
    /// Acción estable compatible con Java.
    pub action: &'static str,
    /// Tipo de entidad afectada.
    pub entity_type: &'static str,
    /// Entidad afectada.
    pub entity_id: EntityId,
    /// Campos modificados, sin valores potencialmente sensibles.
    pub changed_fields: Vec<&'static str>,
}

/// Marca temporal UTC entregada por un reloj de infraestructura.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtcTimestamp(String);

impl UtcTimestamp {
    /// Construye una marca no vacía y libre de NUL. La serialización RFC 3339 se
    /// comprueba en el adaptador que conoce su reloj concreto.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        if value.is_empty() || value.contains('\0') {
            return Err(DomainError::InvalidPersistedValue);
        }
        Ok(Self(value.to_owned()))
    }

    /// Valor persistible de la marca temporal.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reloj inyectable para adaptadores que deban sellar operaciones en UTC.
pub trait Clock {
    /// Devuelve el instante actual. Un fallo se propaga y debe abortar la UoW.
    fn now_utc(&self) -> Result<UtcTimestamp, ApplicationError>;
}

/// Dependencia para generar UUIDs; una implementación productiva no puede reutilizar IDs.
pub trait IdentityGenerator {
    /// Genera una identidad UUID canónica nueva.
    fn next_id(&mut self) -> Result<EntityId, ApplicationError>;
}

/// Decisión contextual; toda implementación debe denegar al no poder decidir.
pub trait Authorization {
    /// Decide un permiso contra su contexto de organización y sucursal opcional.
    fn is_allowed(
        &self,
        permission: Permission,
        organization_id: Option<&EntityId>,
        branch_id: Option<&EntityId>,
    ) -> bool;
}

/// Permisos que estos casos de uso requieren.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permission {
    /// Provisioning inicial, solo por bootstrap explícito.
    BootstrapOrganization,
    /// Lectura de organización.
    OrganizationRead,
    /// Actualización de organización.
    OrganizationUpdate,
    /// Alta de sucursal.
    BranchCreate,
    /// Lectura de sucursales.
    BranchRead,
    /// Actualización de sucursal.
    BranchUpdate,
}

/// Repositorio de organización expresado en operaciones del negocio.
pub trait OrganizationRepository {
    /// Busca por identidad.
    fn get_organization(&mut self, id: &EntityId)
    -> Result<Option<Organization>, ApplicationError>;
    /// Indica si el código global ya está ocupado.
    fn organization_code_exists(&mut self, code: &BusinessCode) -> Result<bool, ApplicationError>;
    /// Inserta una organización nueva.
    fn insert_organization(&mut self, organization: &Organization) -> Result<(), ApplicationError>;
    /// Actualiza condicionalmente por revisión previa; `false` representa CAS perdido.
    fn update_organization_cas(
        &mut self,
        previous_revision: u64,
        organization: &Organization,
    ) -> Result<bool, ApplicationError>;
}

/// Repositorio de sucursal siempre scoped por organización.
pub trait BranchRepository {
    /// Busca una sucursal dentro de su owner; jamás recibe lookup global.
    fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Option<Branch>, ApplicationError>;
    /// Lista solo sucursales del owner solicitado.
    fn list_branches(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Vec<Branch>, ApplicationError>;
    /// Comprueba unicidad del código dentro del owner.
    fn branch_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &BusinessCode,
    ) -> Result<bool, ApplicationError>;
    /// Inserta una sucursal nueva.
    fn insert_branch(&mut self, branch: &Branch) -> Result<(), ApplicationError>;
    /// CAS scoped por owner y revisión previa.
    fn update_branch_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        branch: &Branch,
    ) -> Result<bool, ApplicationError>;
}

/// Auditoría append-only que se ejecuta en la transacción actual.
pub trait AuditAppender {
    /// Añade un evento o falla; el fallo debe abortar la unidad de trabajo.
    fn append_audit(&mut self, event: AuditEvent) -> Result<(), ApplicationError>;
}

/// Contexto mutante transaccional de los casos de uso.
pub trait OrganizationBranchSession:
    OrganizationRepository + BranchRepository + AuditAppender
{
}
impl<T: OrganizationRepository + BranchRepository + AuditAppender> OrganizationBranchSession for T {}

/// Unidad de trabajo sin acoplarla a SQLx ni a un framework.
pub trait UnitOfWork {
    /// Sesión que comparte la transacción de negocio y auditoría.
    type Session: OrganizationBranchSession;
    /// Ejecuta y confirma solo cuando la operación completa devuelve éxito.
    fn transaction<T>(
        &mut self,
        operation: impl FnOnce(&mut Self::Session) -> Result<T, ApplicationError>,
    ) -> Result<T, ApplicationError>;
}

/// Errores de la capa de aplicación, sin códigos HTTP ni detalles de infraestructura.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationError {
    /// La identidad no posee el permiso contextual.
    Forbidden,
    /// Organización inexistente.
    OrganizationNotFound,
    /// Sucursal inexistente en el scope solicitado.
    BranchNotFound,
    /// Código global de organización duplicado.
    OrganizationCodeAlreadyExists,
    /// Código de sucursal duplicado dentro del owner.
    BranchCodeAlreadyExists,
    /// CAS perdido después de haber validado el patch.
    ConcurrentModification,
    /// Fallo de dominio conservando su tipado.
    Domain(DomainError),
    /// Fallo del adaptador sin filtrar detalles sensibles.
    Storage,
    /// Fallo obligatorio de auditoría.
    Audit,
}

impl From<DomainError> for ApplicationError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}

fn organization_changed_fields(before: &Organization, after: &Organization) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name");
    }
    if before.timezone() != after.timezone() {
        fields.push("timezone");
    }
    if before.currency() != after.currency() {
        fields.push("currency");
    }
    if before.locale() != after.locale() {
        fields.push("locale");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    if before.negative_stock_policy() != after.negative_stock_policy() {
        fields.push("inventoryNegativeStockPolicy");
    }
    fields
}

fn branch_changed_fields(before: &Branch, after: &Branch) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name");
    }
    if before.timezone_override() != after.timezone_override() {
        fields.push("timezone");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    fields
}

/// Casos de uso de Organization sobre puertos transaccionales.
pub struct OrganizationApplication<U, A, I> {
    unit_of_work: U,
    authorization: A,
    identities: I,
}

impl<U, A, I> OrganizationApplication<U, A, I>
where
    U: UnitOfWork,
    A: Authorization,
    I: IdentityGenerator,
{
    /// Ensambla los puertos; no instala una política allow-all.
    pub fn new(unit_of_work: U, authorization: A, identities: I) -> Self {
        Self {
            unit_of_work,
            authorization,
            identities,
        }
    }

    /// Crea la organización exclusivamente para un contexto bootstrap autorizado.
    pub fn create(&mut self, input: NewOrganization) -> Result<Organization, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::BootstrapOrganization, None, None)
        {
            return Err(ApplicationError::Forbidden);
        }
        let id = self.identities.next_id()?;
        self.unit_of_work.transaction(|session| {
            if session.organization_code_exists(&input.code)? {
                return Err(ApplicationError::OrganizationCodeAlreadyExists);
            }
            let organization = Organization::create(id, input);
            session.insert_organization(&organization)?;
            Ok(organization)
        })
    }

    /// Recupera una organización tras autorizar su scope.
    pub fn get(&mut self, organization_id: &EntityId) -> Result<Organization, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::OrganizationRead, Some(organization_id), None)
        {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            session
                .get_organization(organization_id)?
                .ok_or(ApplicationError::OrganizationNotFound)
        })
    }

    /// Aplica un patch CAS y audita únicamente una mutación real, como Java.
    pub fn update(
        &mut self,
        organization_id: &EntityId,
        patch: OrganizationPatch,
    ) -> Result<Organization, ApplicationError> {
        if !self.authorization.is_allowed(
            Permission::OrganizationUpdate,
            Some(organization_id),
            None,
        ) {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            let mut organization = session
                .get_organization(organization_id)?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            let before = organization.clone();
            let previous_revision = organization.revision();
            let changed = organization.patch(patch)?;
            if !changed {
                return Ok(organization);
            }
            if !session.update_organization_cas(previous_revision, &organization)? {
                return Err(ApplicationError::ConcurrentModification);
            }
            session.append_audit(AuditEvent {
                organization_id: organization_id.clone(),
                branch_id: None,
                action: "organization.updated",
                entity_type: "organization",
                entity_id: organization_id.clone(),
                changed_fields: organization_changed_fields(&before, &organization),
            })?;
            Ok(organization)
        })
    }

    /// Descompone el servicio una vez configurado por el composition root.
    pub fn into_parts(self) -> (U, A, I) {
        (self.unit_of_work, self.authorization, self.identities)
    }
}

/// Casos de uso de Branch sobre los mismos puertos transaccionales.
pub struct BranchApplication<U, A, I> {
    unit_of_work: U,
    authorization: A,
    identities: I,
}

impl<U, A, I> BranchApplication<U, A, I>
where
    U: UnitOfWork,
    A: Authorization,
    I: IdentityGenerator,
{
    /// Ensambla el caso de uso sin abrir ninguna autorización por defecto.
    pub fn new(unit_of_work: U, authorization: A, identities: I) -> Self {
        Self {
            unit_of_work,
            authorization,
            identities,
        }
    }

    /// Crea y audita una sucursal dentro de una organización existente.
    pub fn create(
        &mut self,
        organization_id: &EntityId,
        input: NewBranch,
    ) -> Result<Branch, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::BranchCreate, Some(organization_id), None)
        {
            return Err(ApplicationError::Forbidden);
        }
        let id = self.identities.next_id()?;
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            if session.branch_code_exists(organization_id, &input.code)? {
                return Err(ApplicationError::BranchCodeAlreadyExists);
            }
            let branch = Branch::create(id, organization_id.clone(), input);
            session.insert_branch(&branch)?;
            session.append_audit(AuditEvent {
                organization_id: organization_id.clone(),
                branch_id: Some(branch.id().clone()),
                action: "branch.created",
                entity_type: "branch",
                entity_id: branch.id().clone(),
                changed_fields: Vec::new(),
            })?;
            Ok(branch)
        })
    }

    /// Lista sucursales únicamente dentro del owner visible.
    pub fn list(&mut self, organization_id: &EntityId) -> Result<Vec<Branch>, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::BranchRead, Some(organization_id), None)
        {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            session.list_branches(organization_id)
        })
    }

    /// Obtiene una sucursal scoped, ocultando IDs de otras organizaciones.
    pub fn get(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Branch, ApplicationError> {
        if !self.authorization.is_allowed(
            Permission::BranchRead,
            Some(organization_id),
            Some(branch_id),
        ) {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            session
                .get_branch(organization_id, branch_id)?
                .ok_or(ApplicationError::BranchNotFound)
        })
    }

    /// Descompone el servicio una vez configurado por el composition root.
    pub fn into_parts(self) -> (U, A, I) {
        (self.unit_of_work, self.authorization, self.identities)
    }

    /// Aplica CAS scoped y registra solo cambios materiales.
    pub fn update(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        patch: BranchPatch,
    ) -> Result<Branch, ApplicationError> {
        if !self.authorization.is_allowed(
            Permission::BranchUpdate,
            Some(organization_id),
            Some(branch_id),
        ) {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            let mut branch = session
                .get_branch(organization_id, branch_id)?
                .ok_or(ApplicationError::BranchNotFound)?;
            let before = branch.clone();
            let previous_revision = branch.revision();
            let changed = branch.patch(organization_id, patch)?;
            if !changed {
                return Ok(branch);
            }
            if !session.update_branch_cas(organization_id, previous_revision, &branch)? {
                return Err(ApplicationError::ConcurrentModification);
            }
            session.append_audit(AuditEvent {
                organization_id: organization_id.clone(),
                branch_id: Some(branch_id.clone()),
                action: "branch.updated",
                entity_type: "branch",
                entity_id: branch_id.clone(),
                changed_fields: branch_changed_fields(&before, &branch),
            })?;
            Ok(branch)
        })
    }
}
