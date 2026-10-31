use crate::{CredentialError, PROVISIONING_TOKEN_HEX_LEN, ProvisioningToken, TokenDigest};

#[test]
fn generated_tokens_are_256_bit_hex_and_unique() -> Result<(), Box<dyn std::error::Error>> {
    let first = ProvisioningToken::generate()?;
    let second = ProvisioningToken::generate()?;
    assert_eq!(first.expose().len(), PROVISIONING_TOKEN_HEX_LEN);
    assert!(
        first
            .expose()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert_ne!(first.expose(), second.expose());
    assert_ne!(first.digest(), second.digest());
    Ok(())
}

#[test]
fn encoded_tokens_round_trip_and_reject_non_canonical_input()
-> Result<(), Box<dyn std::error::Error>> {
    let token = ProvisioningToken::generate()?;
    let restored = ProvisioningToken::from_encoded(token.expose())?;
    assert_eq!(token.expose(), restored.expose());

    for value in [
        String::new(),
        "0".repeat(PROVISIONING_TOKEN_HEX_LEN - 1),
        "0".repeat(PROVISIONING_TOKEN_HEX_LEN + 1),
        "A".repeat(PROVISIONING_TOKEN_HEX_LEN),
        format!("{}g", "0".repeat(PROVISIONING_TOKEN_HEX_LEN - 1)),
    ] {
        assert_eq!(
            ProvisioningToken::from_encoded(&value),
            Err(CredentialError::InvalidInput),
            "valor {value:?}"
        );
    }
    Ok(())
}

#[test]
fn digest_matches_only_the_presented_token() -> Result<(), Box<dyn std::error::Error>> {
    let token = ProvisioningToken::generate()?;
    let other = ProvisioningToken::generate()?;
    let digest = token.digest();

    assert!(digest.matches(&token));
    assert!(!digest.matches(&other));
    assert_eq!(TokenDigest::parse(digest.as_str())?, digest);

    let malformed = vec![
        String::new(),
        "0".repeat(PROVISIONING_TOKEN_HEX_LEN - 1),
        "Z".repeat(PROVISIONING_TOKEN_HEX_LEN),
    ];
    for value in &malformed {
        assert_eq!(
            TokenDigest::parse(value),
            Err(CredentialError::InvalidInput)
        );
    }
    Ok(())
}

#[test]
fn debug_never_reveals_the_token_or_its_digest() -> Result<(), Box<dyn std::error::Error>> {
    let token = ProvisioningToken::generate()?;
    let digest = token.digest();
    assert_eq!(format!("{token:?}"), "ProvisioningToken(<redacted>)");
    assert_eq!(format!("{digest:?}"), "TokenDigest(<redacted>)");
    assert!(!format!("{token:?}").contains(token.expose()));
    assert!(!format!("{digest:?}").contains(digest.as_str()));
    Ok(())
}
