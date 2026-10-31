//! Dominio Organization/Branch y contratos de aplicación compartidos.
//!
//! Contiene las entidades de organización y sucursal con sus invariantes, los
//! value objects comunes, el catálogo de permisos y los puertos que consumen los
//! casos de uso. No conoce SQLx, HTTP ni transporte.
//!
//! # Organización
//!
//! Por responsabilidad: [`errors`], [`ids`], [`text`], [`values`],
//! [`organization`], [`branch`], [`audit_event`], [`permissions`], [`ports`] y
//! [`application`]. La API pública se reexporta aquí, de modo que
//! `pos_core::organization_branch::<Item>` conserva su ruta.

mod audit_event;
mod branch;
mod errors;
mod ids;
mod organization;
mod permissions;
mod ports;
mod text;
mod values;

#[cfg(test)]
mod tests;

pub use audit_event::{AuditEvent, UtcTimestamp};
pub use branch::{Branch, BranchPatch, NewBranch, TimezoneOverridePatch, branch_changed_fields};
pub use errors::{ApplicationError, DomainError};
pub use ids::EntityId;
pub use organization::{
    NewOrganization, Organization, OrganizationPatch, organization_changed_fields,
};
pub use permissions::Permission;
pub use ports::{
    AuditAppender, Authorization, BranchRepository, Clock, IdentityGenerator,
    OrganizationBranchSession, OrganizationRepository, UnitOfWork,
};
pub use text::{BusinessCode, DisplayName, java_trim};
pub use values::{IsoCurrency, LocaleTag, NegativeStockPolicy, Status, Timezone};

/// Mayor revisión representable en el `BIGINT` compartido con Java/PostgreSQL.
///
/// El dominio rechaza incrementos por encima de este valor para no producir un
/// estado que la persistencia no pueda representar sin truncamiento.
pub const MAX_REVISION: u64 = i64::MAX as u64;

pub(crate) fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Reconoce `UTC`, `GMT` o `UT` seguidos de un offset fijo de `ZoneId.of`.
pub(crate) fn valid_prefixed_offset(value: &str) -> bool {
    ["UTC", "GMT", "UT"]
        .iter()
        .any(|prefix| value.strip_prefix(prefix).is_some_and(valid_offset))
}

/// Reconoce las formas de offset fijo que acepta `ZoneId.of` de Java.
///
/// Admite `+H`, `+HH`, `+HHMM`, `+HH:MM`, `+HHMMSS` y `+HH:MM:SS`. El rango se
/// limita a `±18:00:00` con minutos y segundos menores que 60, que es el máximo
/// que Java acepta sin normalizar silenciosamente una entrada ambigua.
pub(crate) fn valid_offset(value: &str) -> bool {
    let Some(rest) = value.strip_prefix('+').or_else(|| value.strip_prefix('-')) else {
        return false;
    };
    // La segmentación por bytes de abajo exige ASCII: una entrada UTF-8 no debe
    // provocar un corte en un límite de carácter inválido.
    if !rest.is_ascii() {
        return false;
    }

    let (hours, minutes, seconds) = if rest.contains(':') {
        let mut parts = rest.split(':');
        let (Some(hours), Some(minutes)) = (parts.next(), parts.next()) else {
            return false;
        };
        let seconds = parts.next().unwrap_or("00");
        if parts.next().is_some() || hours.len() != 2 || minutes.len() != 2 || seconds.len() != 2 {
            return false;
        }
        (hours, minutes, seconds)
    } else {
        match rest.len() {
            1 | 2 => (rest, "00", "00"),
            4 => (&rest[..2], &rest[2..], "00"),
            6 => (&rest[..2], &rest[2..4], &rest[4..]),
            _ => return false,
        }
    };

    let parse = |part: &str| {
        if !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()) {
            part.parse::<u16>().ok()
        } else {
            None
        }
    };
    let (Some(hours), Some(minutes), Some(seconds)) =
        (parse(hours), parse(minutes), parse(seconds))
    else {
        return false;
    };
    hours <= 18 && minutes <= 59 && seconds <= 59 && (hours != 18 || (minutes == 0 && seconds == 0))
}

/// Valida una zona nombrada contra la base TZDB incluida en el binario.
///
/// Una forma textual plausible no basta: `Europe/` o `America//Mexico` comparten
/// sintaxis con zonas reales y deben rechazarse.
pub(crate) fn valid_tzdb_zone(value: &str) -> bool {
    value.parse::<chrono_tz::Tz>().is_ok()
}

pub(crate) const JAVA_CURRENCIES: &[&str] = &[
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
