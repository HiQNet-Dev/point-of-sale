// Pruebas de handles opacos: ninguna salida de diagnóstico debe revelarlos.
#![allow(clippy::panic)]

use std::collections::BTreeSet;

use crate::{CredentialError, SESSION_HANDLE_BYTES, SessionHandle};

#[test]
fn generated_handles_have_full_entropy_and_are_not_reused() {
    let mut seen = BTreeSet::new();
    for _ in 0..64 {
        let handle = SessionHandle::generate().unwrap_or_else(|error| panic!("handle: {error}"));
        let secret = handle.as_client_secret();
        assert_eq!(secret.len(), SESSION_HANDLE_BYTES * 2);
        assert!(
            secret
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        assert!(seen.insert(secret.to_owned()), "handle repetido");
    }
}

#[test]
fn hashes_are_stable_and_independent_of_the_clear_value() {
    let handle = SessionHandle::generate().unwrap_or_else(|error| panic!("handle: {error}"));
    let stored = handle
        .hash()
        .unwrap_or_else(|error| panic!("hash: {error}"));
    assert_eq!(stored.as_storage_str().len(), 64);
    assert_ne!(stored.as_storage_str(), handle.as_client_secret());
    assert_eq!(
        handle.hash().map(|value| value.as_storage_str().to_owned()),
        Ok(stored.as_storage_str().to_owned())
    );
    let reconstructed = SessionHandle::from_encoded(handle.as_client_secret())
        .unwrap_or_else(|error| panic!("handle: {error}"));
    assert_eq!(
        reconstructed
            .hash()
            .map(|value| value.as_storage_str().to_owned()),
        Ok(stored.as_storage_str().to_owned())
    );
}

#[test]
fn invalid_encodings_and_debug_output_never_leak_the_handle() {
    assert_eq!(
        SessionHandle::from_encoded("abc"),
        Err(CredentialError::InvalidInput)
    );
    let uppercase = "A".repeat(SESSION_HANDLE_BYTES * 2);
    assert_eq!(
        SessionHandle::from_encoded(&uppercase),
        Err(CredentialError::InvalidInput)
    );
    let handle = SessionHandle::generate().unwrap_or_else(|error| panic!("handle: {error}"));
    let debug = format!("{handle:?}");
    assert_eq!(debug, "SessionHandle(REDACTED)");
    assert!(!debug.contains(handle.as_client_secret()));
}
