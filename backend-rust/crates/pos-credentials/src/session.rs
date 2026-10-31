//! Handles de sesión Local opacos: generación CSPRNG y hash de almacenamiento.

use std::fmt;

use pos_core::identity::SessionHash;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::CredentialError;

/// Entropía del handle de sesión en bytes (256 bits).
pub const SESSION_HANDLE_BYTES: usize = 32;

/// Handle opaco de sesión.
///
/// El valor en claro solo debe entregarse al cliente por el canal aprobado y
/// nunca persistirse: en base de datos se guarda [`SessionHandle::hash`]. Su
/// `Debug` siempre está redactado.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionHandle(String);

impl SessionHandle {
    /// Genera un handle nuevo con `OsRng`.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::CryptoUnavailable`] si no hay entropía.
    pub fn generate() -> Result<Self, CredentialError> {
        let mut bytes = [0_u8; SESSION_HANDLE_BYTES];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| CredentialError::CryptoUnavailable)?;
        Ok(Self(hex(&bytes)))
    }

    /// Reconstruye un handle desde su representación hexadecimal.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::InvalidInput`] si no son 64 hex minúsculas.
    pub fn from_encoded(value: &str) -> Result<Self, CredentialError> {
        if value.len() != SESSION_HANDLE_BYTES * 2
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(CredentialError::InvalidInput);
        }
        Ok(Self(value.to_owned()))
    }

    /// Hash SHA-256 hexadecimal apto para `local_session.session_hash`.
    ///
    /// # Errors
    /// Devuelve [`CredentialError::CryptoUnavailable`] si el digest no cumple el
    /// contrato de 64 hex minúsculas.
    pub fn hash(&self) -> Result<SessionHash, CredentialError> {
        SessionHash::new(&sha256_hex(self.0.as_bytes()))
            .map_err(|_| CredentialError::CryptoUnavailable)
    }

    /// Representación que el composition root entregará al cliente.
    ///
    /// No debe registrarse ni incluirse en auditoría, telemetría o errores.
    pub fn as_client_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionHandle(REDACTED)")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for &byte in digest.iter() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
