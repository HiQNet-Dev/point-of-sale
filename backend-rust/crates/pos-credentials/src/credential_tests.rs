// Vectores sintéticos y públicos; no son credenciales reutilizables.
#![allow(clippy::panic)]

use pos_core::identity::{CredentialAlgorithm, CredentialKind, SecretHash};

use crate::{
    Argon2Policy, CredentialError, VerificationOutcome, VerifyRequest, hash_password,
    verify_credential,
};

const SPRING_VECTORS: &str =
    include_str!("../../../tools/migration-lab/fixtures/auth/spring-vectors.json");

fn stored(encoded: &str) -> SecretHash {
    SecretHash::new(encoded).unwrap_or_else(|error| panic!("fixture hash: {error}"))
}

fn request<'a>(
    algorithm: CredentialAlgorithm,
    stored: &'a SecretHash,
    password: &'a str,
) -> VerifyRequest<'a> {
    VerifyRequest {
        purpose: CredentialKind::Password,
        algorithm,
        stored,
        password,
    }
}

fn verify(
    algorithm: CredentialAlgorithm,
    stored: &SecretHash,
    password: &str,
) -> Result<VerificationOutcome, CredentialError> {
    verify_credential(&request(algorithm, stored, password))
}

#[test]
fn proposed_commercial_policy_is_not_an_activatable_crypto_policy() {
    // Versión propuesta en ADR-0025: no se registra por desarrollar bootstrap.
    assert_eq!(Argon2Policy::from_id("free-local-argon2id-v1"), None);
    assert_eq!(Argon2Policy::LAB_V1.memory_kib(), 19_456);
    assert_eq!(Argon2Policy::LAB_V1.iterations(), 2);
    assert_eq!(Argon2Policy::LAB_V1.lanes(), 1);
}

#[test]
fn spring_bcrypt_vectors_verify_only_with_their_own_password() {
    let parsed: serde_json::Value =
        serde_json::from_str(SPRING_VECTORS).unwrap_or_else(|error| panic!("vectors: {error}"));
    let vectors = parsed
        .get("vectors")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("vectors array"));
    assert_eq!(vectors.len(), 15);
    for vector in vectors {
        let password = vector
            .get("password")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("password"));
        let encoded = vector
            .get("encoded")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("encoded"));
        let reference = stored(encoded);
        assert_eq!(
            verify(CredentialAlgorithm::LegacyBcrypt, &reference, password),
            Ok(VerificationOutcome::Verified {
                rehash_required: true
            }),
            "vector rechazado"
        );
        assert_eq!(
            verify(
                CredentialAlgorithm::LegacyBcrypt,
                &reference,
                "definitely-not-the-password"
            ),
            Ok(VerificationOutcome::Rejected)
        );
    }
}

#[test]
fn bcrypt_rejects_oversized_nul_and_malformed_material_without_leaking() {
    let reference = stored("$2b$04$WLGfreUeNXWqw6cV3Pq9IOaIeeH/v7O8PxPv/l.Ukazkf8D4nwh.6");
    assert_eq!(
        verify(
            CredentialAlgorithm::LegacyBcrypt,
            &reference,
            &"a".repeat(73)
        ),
        Err(CredentialError::InputTooLong)
    );
    assert!(
        verify(
            CredentialAlgorithm::LegacyBcrypt,
            &reference,
            &"a".repeat(72)
        )
        .is_ok()
    );
    assert_eq!(
        verify(CredentialAlgorithm::LegacyBcrypt, &reference, "a\0b"),
        Err(CredentialError::InvalidInput)
    );
    assert_eq!(
        verify(CredentialAlgorithm::LegacyBcrypt, &reference, ""),
        Err(CredentialError::InvalidInput)
    );
    for malformed in [
        "$2x$04$WLGfreUeNXWqw6cV3Pq9IOaIeeH/v7O8PxPv/l.Ukazkf8D4nwh.6",
        "$2b$13$WLGfreUeNXWqw6cV3Pq9IOaIeeH/v7O8PxPv/l.Ukazkf8D4nwh.6",
        "$2b$04$short",
        "not-a-hash",
    ] {
        assert_eq!(
            verify(
                CredentialAlgorithm::LegacyBcrypt,
                &stored(malformed),
                "irrelevant"
            ),
            Ok(VerificationOutcome::Unusable),
            "material aceptado: {malformed}"
        );
    }
    let message = format!(
        "{:?}",
        request(
            CredentialAlgorithm::LegacyBcrypt,
            &reference,
            "super-secret-value"
        )
    );
    assert!(!message.contains("super-secret-value"));
    assert!(!message.contains("WLGfreUe"));
}

#[test]
fn argon2id_hash_verify_and_policy_detection_are_exact() {
    let hash = hash_password("lab-only-passphrase", Argon2Policy::LAB_V1)
        .unwrap_or_else(|error| panic!("hash: {error}"));
    assert!(
        hash.expose_to_verifier()
            .starts_with("$argon2id$v=19$m=19456,t=2,p=1$")
    );
    assert_eq!(
        verify(CredentialAlgorithm::Argon2id, &hash, "lab-only-passphrase"),
        Ok(VerificationOutcome::Verified {
            rehash_required: false
        })
    );
    assert_eq!(
        verify(CredentialAlgorithm::Argon2id, &hash, "lab-only-passphrasf"),
        Ok(VerificationOutcome::Rejected)
    );

    let older = hash_password("lab-only-passphrase", Argon2Policy::LAB_V0)
        .unwrap_or_else(|error| panic!("hash: {error}"));
    assert_eq!(
        verify(CredentialAlgorithm::Argon2id, &older, "lab-only-passphrase"),
        Ok(VerificationOutcome::Verified {
            rehash_required: true
        })
    );

    let tampered = hash.expose_to_verifier().replace("m=19456", "m=20480");
    assert_eq!(
        verify(
            CredentialAlgorithm::Argon2id,
            &stored(&tampered),
            "lab-only-passphrase"
        ),
        Ok(VerificationOutcome::Unusable)
    );
    let tampered = hash.expose_to_verifier().replace(",t=2,", ",t=2,p=2,");
    assert_eq!(
        verify(
            CredentialAlgorithm::Argon2id,
            &stored(&tampered),
            "lab-only-passphrase"
        ),
        Ok(VerificationOutcome::Unusable)
    );
    for malformed in [
        "$argon2id$broken",
        "$argon2i$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
    ] {
        assert_eq!(
            verify(
                CredentialAlgorithm::Argon2id,
                &stored(malformed),
                "irrelevant"
            ),
            Ok(VerificationOutcome::Unusable)
        );
    }
}

#[test]
fn argon2id_salts_are_unique_and_hashes_are_not_reused() {
    let first = hash_password("same-passphrase", Argon2Policy::LAB_V1)
        .unwrap_or_else(|error| panic!("hash: {error}"));
    let second = hash_password("same-passphrase", Argon2Policy::LAB_V1)
        .unwrap_or_else(|error| panic!("hash: {error}"));
    assert_ne!(first, second);
    assert_eq!(
        verify(CredentialAlgorithm::Argon2id, &first, "same-passphrase"),
        verify(CredentialAlgorithm::Argon2id, &second, "same-passphrase")
    );
    assert_eq!(
        hash_password("", Argon2Policy::LAB_V1),
        Err(CredentialError::InvalidInput)
    );
    assert_eq!(
        hash_password("a\0b", Argon2Policy::LAB_V1),
        Err(CredentialError::InvalidInput)
    );
    assert_eq!(
        hash_password(&"a".repeat(1_025), Argon2Policy::LAB_V1),
        Err(CredentialError::InputTooLong)
    );
}

#[test]
fn policy_catalog_is_closed_and_ids_resolve() {
    assert_eq!(
        Argon2Policy::from_id("lab-argon2-v1"),
        Some(Argon2Policy::LAB_V1)
    );
    assert_eq!(Argon2Policy::from_id("lab-argon2-v9"), None);
    assert_eq!(Argon2Policy::LAB_V1.id(), "lab-argon2-v1");
    assert_eq!(Argon2Policy::LAB_V1.memory_kib(), 19_456);
    assert_eq!(Argon2Policy::LAB_V1.iterations(), 2);
    assert_eq!(Argon2Policy::LAB_V1.lanes(), 1);
    assert_eq!(Argon2Policy::LAB_V0.id(), "lab-argon2-v0");
}
