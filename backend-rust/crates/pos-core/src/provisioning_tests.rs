use crate::{
    identity::{
        MAX_PROVISIONING_TTL_SECONDS, MIN_PROVISIONING_TTL_SECONDS, ProvisioningAuthorization,
        ProvisioningError, ProvisioningPurpose, ProvisioningState,
    },
    organization_branch::EntityId,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn id(value: u128) -> Result<EntityId, Box<dyn std::error::Error>> {
    Ok(EntityId::parse(&format!(
        "00000000-0000-0000-0000-{value:012x}"
    ))?)
}

fn digest(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn authorization(token: &str) -> Result<ProvisioningAuthorization, Box<dyn std::error::Error>> {
    Ok(ProvisioningAuthorization::issue(
        id(1)?,
        id(2)?,
        ProvisioningPurpose::Bootstrap,
        token,
        1_000,
        1_600,
    )?)
}

#[test]
fn issue_rejects_malformed_digests_and_out_of_policy_windows() -> TestResult {
    let malformed = vec![
        String::new(),
        digest('a')[..63].to_owned(),
        digest('A'),
        "z".repeat(64),
        format!("g{}", "0".repeat(63)),
    ];
    for value in &malformed {
        assert_eq!(
            ProvisioningAuthorization::issue(
                id(1)?,
                id(2)?,
                ProvisioningPurpose::Bootstrap,
                value,
                1_000,
                1_600
            ),
            Err(ProvisioningError::InvalidDigest),
            "digest {value:?}"
        );
    }
    let token = digest('a');
    for (issued_at, expires_at) in [
        (1_000, 1_000 + MIN_PROVISIONING_TTL_SECONDS - 1),
        (1_000, 1_000 + MAX_PROVISIONING_TTL_SECONDS + 1),
        (1_000, 999),
        (i64::MIN, i64::MAX),
    ] {
        assert_eq!(
            ProvisioningAuthorization::issue(
                id(1)?,
                id(2)?,
                ProvisioningPurpose::Bootstrap,
                &token,
                issued_at,
                expires_at
            ),
            Err(ProvisioningError::InvalidWindow),
            "ventana {issued_at}..{expires_at}"
        );
    }
    for ttl in [MIN_PROVISIONING_TTL_SECONDS, MAX_PROVISIONING_TTL_SECONDS] {
        ProvisioningAuthorization::issue(
            id(1)?,
            id(2)?,
            ProvisioningPurpose::Bootstrap,
            &token,
            1_000,
            1_000 + ttl,
        )?;
    }
    Ok(())
}

#[test]
fn authorize_accepts_only_the_bound_presentation() -> TestResult {
    let token = digest('a');
    let authorization = authorization(&token)?;
    let installation = id(2)?;

    assert_eq!(
        authorization.authorize(&token, &installation, ProvisioningPurpose::Bootstrap, 1_200),
        Ok(())
    );
    assert_eq!(authorization.state(), ProvisioningState::Pending);
    assert_eq!(
        authorization.authorize(
            &digest('b'),
            &installation,
            ProvisioningPurpose::Bootstrap,
            1_200
        ),
        Err(ProvisioningError::TokenMismatch)
    );
    assert_eq!(
        authorization.authorize(&token, &id(3)?, ProvisioningPurpose::Bootstrap, 1_200),
        Err(ProvisioningError::InstallationMismatch)
    );
    assert_eq!(
        authorization.authorize(
            &token,
            &installation,
            ProvisioningPurpose::AdministrativeRecovery,
            1_200
        ),
        Err(ProvisioningError::PurposeMismatch)
    );
    assert_eq!(
        authorization.authorize(&token, &installation, ProvisioningPurpose::Bootstrap, 1_600),
        Err(ProvisioningError::Expired)
    );
    Ok(())
}

#[test]
fn consume_is_single_use_and_blocks_replay() -> TestResult {
    let token = digest('a');
    let installation = id(2)?;
    let mut authorization = authorization(&token)?;

    authorization.consume(&token, &installation, ProvisioningPurpose::Bootstrap, 1_200)?;
    assert_eq!(authorization.state(), ProvisioningState::Consumed);
    assert_eq!(authorization.consumed_at(), Some(1_200));
    assert_eq!(
        authorization.consume(&token, &installation, ProvisioningPurpose::Bootstrap, 1_200),
        Err(ProvisioningError::AlreadyConsumed)
    );
    assert_eq!(
        authorization.authorize(&token, &installation, ProvisioningPurpose::Bootstrap, 1_200),
        Err(ProvisioningError::AlreadyConsumed)
    );
    assert_eq!(
        authorization.revoke(1_300),
        Err(ProvisioningError::AlreadyConsumed)
    );
    Ok(())
}

#[test]
fn a_rejected_consume_leaves_the_authorization_untouched() -> TestResult {
    let token = digest('a');
    let mut authorization = authorization(&token)?;
    let before = authorization.clone();

    assert_eq!(
        authorization.consume(&digest('b'), &id(2)?, ProvisioningPurpose::Bootstrap, 1_200),
        Err(ProvisioningError::TokenMismatch)
    );
    assert_eq!(authorization, before);
    assert_eq!(authorization.state(), ProvisioningState::Pending);
    Ok(())
}

#[test]
fn revoke_is_idempotent_and_permanent() -> TestResult {
    let token = digest('a');
    let installation = id(2)?;
    let mut authorization = authorization(&token)?;

    authorization.revoke(1_100)?;
    authorization.revoke(1_150)?;
    assert_eq!(authorization.state(), ProvisioningState::Revoked);
    assert_eq!(authorization.revoked_at(), Some(1_100));
    assert_eq!(
        authorization.authorize(&token, &installation, ProvisioningPurpose::Bootstrap, 1_200),
        Err(ProvisioningError::Revoked)
    );
    assert_eq!(
        authorization.consume(&token, &installation, ProvisioningPurpose::Bootstrap, 1_200),
        Err(ProvisioningError::Revoked)
    );
    Ok(())
}

#[test]
fn rehydrate_restores_state_and_rejects_incoherent_timestamps() -> TestResult {
    let token = digest('a');
    let restored = ProvisioningAuthorization::rehydrate(
        id(1)?,
        id(2)?,
        ProvisioningPurpose::Bootstrap,
        &token,
        1_000,
        1_600,
        Some(1_200),
        None,
    )?;
    assert_eq!(restored.state(), ProvisioningState::Consumed);

    assert_eq!(
        ProvisioningAuthorization::rehydrate(
            id(1)?,
            id(2)?,
            ProvisioningPurpose::Bootstrap,
            &token,
            1_000,
            1_600,
            Some(999),
            None,
        ),
        Err(ProvisioningError::InvalidWindow)
    );
    assert_eq!(
        ProvisioningAuthorization::rehydrate(
            id(1)?,
            id(2)?,
            ProvisioningPurpose::Bootstrap,
            &token,
            1_000,
            1_600,
            None,
            Some(999),
        ),
        Err(ProvisioningError::InvalidWindow)
    );
    Ok(())
}

#[test]
fn purposes_round_trip_and_reject_unknown_values() -> TestResult {
    for purpose in [
        ProvisioningPurpose::Bootstrap,
        ProvisioningPurpose::AdministrativeRecovery,
    ] {
        assert_eq!(ProvisioningPurpose::parse(purpose.as_str()), Ok(purpose));
    }
    assert_eq!(
        ProvisioningPurpose::parse("bootstrap"),
        Err(ProvisioningError::InvalidPurpose)
    );
    assert_eq!(
        ProvisioningPurpose::parse(""),
        Err(ProvisioningError::InvalidPurpose)
    );
    Ok(())
}
