//! Tokens de provisioning administrativo: entropía CSPRNG y digest persistible.
//!
//! El token en claro solo debe entregarse por el canal administrativo aprobado y
//! nunca persistirse: en base de datos se guarda [`TokenDigest`]. Su `Debug`
//! siempre está redactado y ninguna API pública lo imprime.

use std::fmt;

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::CredentialError;

/// Entropía del token de provisioning en bytes (256 bits).
pub const PROVISIONING_TOKEN_BYTES: usize = 32;
/// Longitud del token y de su digest en hexadecimal minúsculo.
pub const PROVISIONING_TOKEN_HEX_LEN: usize = PROVISIONING_TOKEN_BYTES * 2;

/// Token de provisioning opaco, de un solo uso.
///
/// Se genera con `OsRng`; no se deriva de datos del cliente ni de una semilla
/// conocida. El valor en claro solo se entrega una vez por el canal
/// administrativo y se compara siempre a través de [`ProvisioningToken::digest`].
#[derive(Clone, Eq, PartialEq)]
pub struct ProvisioningToken(String);

impl ProvisioningToken {
    /// Genera un token nuevo con 256 bits de entropía.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::CryptoUnavailable`] si no hay entropía.
    pub fn generate() -> Result<Self, CredentialError> {
        let mut bytes = [0_u8; PROVISIONING_TOKEN_BYTES];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| CredentialError::CryptoUnavailable)?;
        Ok(Self(hex(&bytes)))
    }

    /// Reconstruye un token desde su representación hexadecimal.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::InvalidInput`] si no son 64 hex minúsculas.
    pub fn from_encoded(value: &str) -> Result<Self, CredentialError> {
        if !is_hex64(value) {
            return Err(CredentialError::InvalidInput);
        }
        Ok(Self(value.to_owned()))
    }

    /// Representación para el canal administrativo; nunca persistir en claro.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Digest SHA-256 hexadecimal apto para la columna `token_digest`.
    pub fn digest(&self) -> TokenDigest {
        TokenDigest(sha256_hex(self.0.as_bytes()))
    }
}

impl fmt::Debug for ProvisioningToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProvisioningToken(<redacted>)")
    }
}

/// Digest SHA-256 hexadecimal del token; es lo único que se persiste.
#[derive(Clone, Eq, PartialEq)]
pub struct TokenDigest(String);

impl TokenDigest {
    /// Valida un digest persistido de 64 hex minúsculas.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::InvalidInput`] para cualquier otra forma.
    pub fn parse(value: &str) -> Result<Self, CredentialError> {
        if !is_hex64(value) {
            return Err(CredentialError::InvalidInput);
        }
        Ok(Self(value.to_owned()))
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compara contra un token presentado sin salida temprana por prefijo.
    pub fn matches(&self, token: &ProvisioningToken) -> bool {
        constant_time_eq(self.0.as_bytes(), token.digest().0.as_bytes())
    }
}

impl fmt::Debug for TokenDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TokenDigest(<redacted>)")
    }
}

fn is_hex64(value: &str) -> bool {
    value.len() == PROVISIONING_TOKEN_HEX_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Compara dos buffers del mismo tamaño sin cortocircuito por diferencia.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        encoded.push(nibble(byte >> 4));
        encoded.push(nibble(byte & 0x0f));
    }
    encoded
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(nibble(byte >> 4));
        encoded.push(nibble(byte & 0x0f));
    }
    encoded
}

fn nibble(value: u8) -> char {
    char::from_digit(u32::from(value & 0x0f), 16).unwrap_or('0')
}
