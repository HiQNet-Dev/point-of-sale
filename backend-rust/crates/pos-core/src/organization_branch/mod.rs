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

mod application;
mod audit_event;
mod branch;
mod errors;
mod ids;
mod organization;
mod permissions;
mod ports;
mod text;
mod values;

pub use application::{BranchApplication, OrganizationApplication};
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

pub(crate) fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

pub(crate) fn valid_prefixed_offset(value: &str) -> bool {
    for prefix in ["UTC", "GMT", "UT"] {
        if let Some(offset) = value.strip_prefix(prefix) {
            return valid_offset(offset);
        }
    }
    false
}

pub(crate) fn valid_offset(value: &str) -> bool {
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

pub(crate) fn valid_tzdb_shape(value: &str) -> bool {
    value.contains('/')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'.')
                })
        })
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
