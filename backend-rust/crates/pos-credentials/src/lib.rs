//! Frontera criptográfica de credenciales Local (laboratorio 1H).
//!
//! # Alcance
//!
//! Este crate verifica y produce material de credenciales **sin** SQLx, sin
//! transporte y sin secretos en claro almacenados. No implementa login, entrega
//! de sesiones a clientes, JWT ni integración HTTP/IPC.
//!
//! # Reglas
//!
//! - El algoritmo nunca se deduce del hash: se declara con
//!   [`pos_core::identity::CredentialAlgorithm`].
//! - BCrypt legado solo se verifica; Argon2id es el destino de rehash.
//! - Los parámetros son explícitos ([`Argon2Policy`]); no se usa
//!   `Params::default()`.
//! - Los errores no contienen entrada, hash ni material criptográfico.
//! - Ningún tipo público expone la contraseña o el hash mediante `Debug`.
//!
//! Todo parámetro de este crate es **experimental de laboratorio**: F4–19 no ha
//! aprobado parámetros comerciales ni el hardware objetivo.

use std::fmt;

mod policy;
mod provisioning;
mod session;
mod verify;

pub use policy::{
    ARGON2_OUTPUT_BYTES, ARGON2_SALT_BYTES, Argon2Policy, MAX_ARGON2_ITERATIONS, MAX_ARGON2_LANES,
    MAX_ARGON2_MEMORY_KIB, MAX_BCRYPT_COST, MAX_BCRYPT_PASSWORD_BYTES, MAX_PASSWORD_BYTES,
    MAX_PHC_BYTES,
};
pub use provisioning::{
    PROVISIONING_TOKEN_BYTES, PROVISIONING_TOKEN_HEX_LEN, ProvisioningToken, TokenDigest,
    random_uuid_v4,
};
pub use session::{SESSION_HANDLE_BYTES, SessionHandle};
pub use verify::{VerificationOutcome, VerifyRequest, hash_password, verify_credential};

/// Fallo de configuración o de entrada; nunca contiene material sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialError {
    /// Entrada vacía, con NUL o con forma no admitida.
    InvalidInput,
    /// La entrada supera el límite del algoritmo.
    InputTooLong,
    /// Parámetros fuera de la política de laboratorio.
    PolicyRejected,
    /// No se pudo obtener entropía o producir el material.
    CryptoUnavailable,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "entrada de credencial inválida",
            Self::InputTooLong => "entrada de credencial demasiado larga",
            Self::PolicyRejected => "parámetros criptográficos fuera de política",
            Self::CryptoUnavailable => "material criptográfico no disponible",
        })
    }
}

impl std::error::Error for CredentialError {}

#[cfg(test)]
mod credential_tests;
#[cfg(test)]
mod provisioning_tests;
#[cfg(test)]
mod session_tests;
