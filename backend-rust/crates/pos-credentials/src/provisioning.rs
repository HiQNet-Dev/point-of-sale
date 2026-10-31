//! Tokens de provisioning administrativo: entropía CSPRNG y digest persistible.
//!
//! El token en claro solo debe entregarse por el canal administrativo aprobado y
//! nunca persistirse: en base de datos se guarda [`TokenDigest`]. Su `Debug`
//! está redactado, no implementa `Display` ni `Clone`, y el búfer se limpia al
//! liberarse con `Zeroizing`.

use std::fmt;

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::CredentialError;

/// Entropía del token de provisioning en bytes (256 bits).
pub const PROVISIONING_TOKEN_BYTES: usize = 32;
/// Longitud del token y de su digest en hexadecimal minúsculo.
pub const PROVISIONING_TOKEN_HEX_LEN: usize = PROVISIONING_TOKEN_BYTES * 2;

/// Token de provisioning opaco, de un solo uso.
///
/// Se genera con `OsRng`; no se deriva de datos del cliente ni de una semilla
/// conocida. No clona: moverlo es la única forma de transferirlo, y su memoria
/// se limpia al liberarse.
pub struct ProvisioningToken(Zeroizing<String>);

impl ProvisioningToken {
    /// Genera un token nuevo con 256 bits de entropía.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::CryptoUnavailable`] si no hay entropía.
    pub fn generate() -> Result<Self, CredentialError> {
        let mut bytes = Zeroizing::new([0_u8; PROVISIONING_TOKEN_BYTES]);
        OsRng
            .try_fill_bytes(bytes.as_mut())
            .map_err(|_| CredentialError::CryptoUnavailable)?;
        Ok(Self(Zeroizing::new(hex(bytes.as_ref()))))
    }

    /// Reconstruye un token desde su representación hexadecimal.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::InvalidInput`] si no son 64 hex minúsculas.
    pub fn from_encoded(value: &str) -> Result<Self, CredentialError> {
        if !is_hex64(value) {
            return Err(CredentialError::InvalidInput);
        }
        Ok(Self(Zeroizing::new(value.to_owned())))
    }

    /// Representación para el canal administrativo; nunca persistir en claro.
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    /// Digest SHA-256 del token, apto para la columna `token_digest`.
    pub fn digest(&self) -> TokenDigest {
        TokenDigest::of(self.0.as_bytes())
    }
}

/// Genera un identificador canónico UUID v4 con `OsRng`.
///
/// Se usa para identidades de instalación y autorización que nunca elige el
/// cliente.
///
/// # Errors
/// Devuelve [`CredentialError::CryptoUnavailable`] si no hay entropía.
pub fn random_uuid_v4() -> Result<String, CredentialError> {
    let mut bytes = Zeroizing::new([0_u8; 16]);
    OsRng
        .try_fill_bytes(bytes.as_mut())
        .map_err(|_| CredentialError::CryptoUnavailable)?;
    let mut value = *bytes;
    value[6] = (value[6] & 0x0f) | 0x40;
    value[8] = (value[8] & 0x3f) | 0x80;
    let hex = hex(&value);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

impl fmt::Debug for ProvisioningToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProvisioningToken(<redacted>)")
    }
}

/// Digest SHA-256 del token; es lo único que se persiste.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TokenDigest {
    bytes: [u8; PROVISIONING_TOKEN_BYTES],
}

impl TokenDigest {
    fn of(input: &[u8]) -> Self {
        let digest = Sha256::digest(input);
        let mut bytes = [0_u8; PROVISIONING_TOKEN_BYTES];
        bytes.copy_from_slice(&digest);
        Self { bytes }
    }

    /// Valida un digest persistido de 64 hex minúsculas.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::InvalidInput`] para cualquier otra forma.
    pub fn parse(value: &str) -> Result<Self, CredentialError> {
        if !is_hex64(value) {
            return Err(CredentialError::InvalidInput);
        }
        let mut bytes = [0_u8; PROVISIONING_TOKEN_BYTES];
        for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let high = nibble(pair[0]).ok_or(CredentialError::InvalidInput)?;
            let low = nibble(pair[1]).ok_or(CredentialError::InvalidInput)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self { bytes })
    }

    /// Representación persistible en hexadecimal minúsculo.
    pub fn hex(&self) -> String {
        hex(&self.bytes)
    }

    /// Compara contra un token presentado con tiempo constante.
    pub fn matches(&self, token: &ProvisioningToken) -> bool {
        let presented = TokenDigest::of(token.0.as_bytes());
        bool::from(self.bytes.ct_eq(&presented.bytes))
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

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(digit(byte >> 4));
        encoded.push(digit(byte & 0x0f));
    }
    encoded
}

fn digit(value: u8) -> char {
    char::from_digit(u32::from(value & 0x0f), 16).unwrap_or('0')
}
