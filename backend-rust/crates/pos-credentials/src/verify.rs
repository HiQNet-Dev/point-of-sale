//! Verificación y generación de material de credenciales.
//!
//! No hay `unwrap` ni mensajes que incluyan el hash recibido: los errores de
//! bibliotecas criptográficas se colapsan en resultados seguros.

use argon2::{
    Algorithm, Argon2, Version,
    password_hash::{
        Error as HashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::OsRng,
    },
};
use pos_core::identity::{CredentialAlgorithm, CredentialKind, SecretHash};

use crate::{
    CredentialError,
    policy::{
        ARGON2_OUTPUT_BYTES, ARGON2_SALT_BYTES, Argon2Policy, MAX_BCRYPT_COST,
        MAX_BCRYPT_PASSWORD_BYTES, MAX_PASSWORD_BYTES, MAX_PHC_BYTES,
    },
};

/// Solicitud de verificación contra material ya almacenado.
#[derive(Clone, Copy)]
pub struct VerifyRequest<'a> {
    /// Finalidad declarada de la credencial.
    ///
    /// La verificación criptográfica no cambia por finalidad; el caso de uso
    /// decide si un PIN o una contraseña son aceptables para la operación.
    pub purpose: CredentialKind,
    /// Algoritmo declarado por la fila, nunca deducido del hash.
    pub algorithm: CredentialAlgorithm,
    /// Hash codificado almacenado.
    pub stored: &'a SecretHash,
    /// Secreto presentado por el usuario; nunca se registra.
    pub password: &'a str,
}

impl std::fmt::Debug for VerifyRequest<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("VerifyRequest(REDACTED)")
    }
}

/// Resultado seguro de una verificación.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationOutcome {
    /// El secreto coincide; `rehash_required` indica que el material debe migrarse.
    Verified {
        /// `true` para BCrypt legado o Argon2id con política anterior.
        rehash_required: bool,
    },
    /// El secreto no coincide.
    Rejected,
    /// El material almacenado no es verificable: exige reset, sin fallback.
    Unusable,
}

/// Verifica un secreto contra el hash almacenado.
///
/// # Errors
/// Devuelve [`CredentialError::InvalidInput`] o [`CredentialError::InputTooLong`]
/// antes de invocar al verificador criptográfico. Un hash corrupto produce
/// [`VerificationOutcome::Unusable`], nunca un error con contenido.
pub fn verify_credential(
    request: &VerifyRequest<'_>,
) -> Result<VerificationOutcome, CredentialError> {
    validate_secret(request.password, MAX_PASSWORD_BYTES)?;
    let encoded = request.stored.expose_to_verifier();
    match request.algorithm {
        CredentialAlgorithm::LegacyBcrypt => verify_bcrypt(request.password, encoded),
        CredentialAlgorithm::Argon2id => verify_argon2(request.password, encoded),
    }
}

/// Produce un hash Argon2id nuevo con sal CSPRNG.
///
/// # Errors
/// Devuelve [`CredentialError::InvalidInput`], [`CredentialError::InputTooLong`],
/// [`CredentialError::PolicyRejected`] o [`CredentialError::CryptoUnavailable`].
pub fn hash_password(password: &str, policy: Argon2Policy) -> Result<SecretHash, CredentialError> {
    validate_secret(password, MAX_PASSWORD_BYTES)?;
    let params = policy.params()?;
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let hash = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| CredentialError::CryptoUnavailable)?;
    SecretHash::new(&hash.to_string()).map_err(|_| CredentialError::CryptoUnavailable)
}

fn validate_secret(secret: &str, max_bytes: usize) -> Result<(), CredentialError> {
    if secret.is_empty() || secret.contains('\0') {
        return Err(CredentialError::InvalidInput);
    }
    if secret.len() > max_bytes {
        return Err(CredentialError::InputTooLong);
    }
    Ok(())
}

fn verify_bcrypt(secret: &str, encoded: &str) -> Result<VerificationOutcome, CredentialError> {
    // BCrypt opera sobre bytes: el gate de 72 bytes es previo y explícito.
    validate_secret(secret, MAX_BCRYPT_PASSWORD_BYTES)?;
    let supported_variant =
        encoded.starts_with("$2a$") || encoded.starts_with("$2b$") || encoded.starts_with("$2y$");
    let cost = encoded
        .get(4..6)
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|cost| (4..=MAX_BCRYPT_COST).contains(cost));
    if !supported_variant || cost.is_none() || encoded.len() != 60 {
        return Ok(VerificationOutcome::Unusable);
    }
    match bcrypt::verify(secret, encoded) {
        Ok(true) => Ok(VerificationOutcome::Verified {
            rehash_required: true,
        }),
        Ok(false) => Ok(VerificationOutcome::Rejected),
        // Los errores de bcrypt pueden incluir el hash; se colapsan sin él.
        Err(_) => Ok(VerificationOutcome::Unusable),
    }
}

fn salt_matches_contract(salt: &argon2::password_hash::Salt<'_>) -> bool {
    let mut buffer = [0_u8; 64];
    matches!(
        salt.decode_b64(&mut buffer),
        Ok(bytes) if bytes.len() == ARGON2_SALT_BYTES
    )
}

fn verify_argon2(secret: &str, encoded: &str) -> Result<VerificationOutcome, CredentialError> {
    if encoded.len() > MAX_PHC_BYTES {
        return Ok(VerificationOutcome::Unusable);
    }
    let Ok(parsed) = PasswordHash::new(encoded) else {
        return Ok(VerificationOutcome::Unusable);
    };
    if parsed.algorithm.as_str() != "argon2id" || parsed.version != Some(19) {
        return Ok(VerificationOutcome::Unusable);
    }
    if parsed
        .hash
        .as_ref()
        .is_none_or(|hash| hash.len() != ARGON2_OUTPUT_BYTES)
        || parsed
            .salt
            .as_ref()
            .is_none_or(|salt| !salt_matches_contract(salt))
    {
        return Ok(VerificationOutcome::Unusable);
    }
    let Some(policy) = [Argon2Policy::LAB_V0, Argon2Policy::LAB_V1]
        .into_iter()
        .find(|policy| policy.matches(&parsed.params))
    else {
        return Ok(VerificationOutcome::Unusable);
    };
    let Ok(params) = policy.params() else {
        return Ok(VerificationOutcome::Unusable);
    };
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    match argon.verify_password(secret.as_bytes(), &parsed) {
        Ok(()) => Ok(VerificationOutcome::Verified {
            rehash_required: policy != Argon2Policy::LAB_V1,
        }),
        Err(HashError::Password) => Ok(VerificationOutcome::Rejected),
        Err(_) => Ok(VerificationOutcome::Unusable),
    }
}
