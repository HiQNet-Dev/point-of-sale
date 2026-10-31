//! Intercambio de vectores con el verificador Java.
//!
//! Protocolo de líneas separadas por tabulador, sin JSON, para que la
//! comparación entre implementaciones no dependa de un parser común.
//!
//! - `verify-bcrypt`: `label \t password \t hash` → `label \t VERIFIED_REHASH|REJECTED|UNUSABLE`
//! - `emit-argon2`:   `label \t password`          → `label \t phc`
//! - `verify-argon2`: `label \t password \t phc`   → `label \t VERIFIED_REHASH|VERIFIED|REJECTED|UNUSABLE`
//!
//! Los secretos de entrada son sintéticos y no se registran.

use std::io::{self, BufRead, Write};

use pos_core::identity::{CredentialAlgorithm, CredentialKind, SecretHash};
use pos_credentials::{
    Argon2Policy, VerificationOutcome, VerifyRequest, hash_password, verify_credential,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());
    for line in stdin.lock().lines() {
        let line = line?;
        let fields: Vec<&str> = line.split('\t').collect();
        match (mode.as_str(), fields.as_slice()) {
            ("verify-bcrypt", [label, password, encoded]) => {
                let outcome = verify(CredentialAlgorithm::LegacyBcrypt, password, encoded);
                writeln!(stdout, "{label}\t{}", code(outcome))?;
            }
            ("verify-argon2", [label, password, encoded]) => {
                let outcome = verify(CredentialAlgorithm::Argon2id, password, encoded);
                writeln!(stdout, "{label}\t{}", code(outcome))?;
            }
            ("emit-argon2", [label, password]) => {
                // LAB_V1 es el destino de rehash experimental; no es una política
                // comercial aprobada (D-AUTH-09 sigue bloqueada).
                let hash = hash_password(password, Argon2Policy::LAB_V1)?;
                writeln!(stdout, "{label}\t{}", hash.expose_to_verifier())?;
            }
            _ => {}
        }
    }
    stdout.flush()?;
    Ok(())
}

fn verify(
    algorithm: CredentialAlgorithm,
    password: &str,
    encoded: &str,
) -> Result<VerificationOutcome, pos_credentials::CredentialError> {
    let Ok(stored) = SecretHash::new(encoded) else {
        return Ok(VerificationOutcome::Unusable);
    };
    verify_credential(&VerifyRequest {
        purpose: CredentialKind::Password,
        algorithm,
        stored: &stored,
        password,
    })
}

fn code(outcome: Result<VerificationOutcome, pos_credentials::CredentialError>) -> &'static str {
    match outcome {
        Ok(VerificationOutcome::Verified {
            rehash_required: true,
        }) => "VERIFIED_REHASH",
        Ok(VerificationOutcome::Verified {
            rehash_required: false,
        }) => "VERIFIED",
        Ok(VerificationOutcome::Rejected) => "REJECTED",
        Ok(VerificationOutcome::Unusable) => "UNUSABLE",
        Err(_) => "INVALID_INPUT",
    }
}
