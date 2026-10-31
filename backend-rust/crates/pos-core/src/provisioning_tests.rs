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
fn expiry_helper_rejects_overflow_and_out_of_policy_ttl() {
    assert_eq!(
        ProvisioningAuthorization::expires_from(i64::MAX - 10, 60),
        Err(ProvisioningError::InvalidWindow)
    );
    assert_eq!(
        ProvisioningAuthorization::expires_from(1_000, MAX_PROVISIONING_TTL_SECONDS + 1),
        Err(ProvisioningError::InvalidWindow)
    );
    assert_eq!(
        ProvisioningAuthorization::expires_from(1_000, 600),
        Ok(1_600)
    );
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
    assert_eq!(authorization.state_at(1_200), ProvisioningState::Pending);
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
    assert_eq!(
        authorization.authorize(&token, &installation, ProvisioningPurpose::Bootstrap, 999),
        Err(ProvisioningError::ClockBeforeIssue)
    );
    Ok(())
}

#[test]
fn expiry_is_terminal_and_deterministic() -> TestResult {
    let token = digest('a');
    let mut authorization = authorization(&token)?;

    assert_eq!(authorization.state_at(1_599), ProvisioningState::Pending);
    assert_eq!(authorization.state_at(1_600), ProvisioningState::Expired);
    assert!(authorization.is_live_at(1_599));
    assert!(!authorization.is_live_at(1_600));

    // La caducidad es perezosa hasta que se materializa.
    assert_eq!(authorization.state(), ProvisioningState::Pending);
    assert!(authorization.expire(1_700));
    assert_eq!(authorization.state(), ProvisioningState::Expired);
    assert_eq!(authorization.closed_at(), Some(1_600));
    assert!(!authorization.expire(1_800));
    assert_eq!(
        authorization.authorize(&token, &id(2)?, ProvisioningPurpose::Bootstrap, 1_200),
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
    assert_eq!(authorization.closed_at(), Some(1_200));
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
    assert_eq!(authorization.closed_at(), Some(1_100));
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
fn revoking_an_expired_authorization_keeps_the_terminal_state() -> TestResult {
    let token = digest('a');
    let mut authorization = authorization(&token)?;
    assert!(authorization.expire(1_600));
    authorization.revoke(1_700)?;
    assert_eq!(authorization.state(), ProvisioningState::Expired);
    assert_eq!(authorization.closed_at(), Some(1_600));
    Ok(())
}

#[test]
fn rehydrate_restores_terminal_states_and_rejects_incoherence() -> TestResult {
    let token = digest('a');
    for (state, closed_at) in [
        (ProvisioningState::Consumed, Some(1_200)),
        (ProvisioningState::Revoked, Some(1_200)),
        (ProvisioningState::Expired, Some(1_600)),
    ] {
        let restored = ProvisioningAuthorization::rehydrate(
            id(1)?,
            id(2)?,
            ProvisioningPurpose::Bootstrap,
            &token,
            1_000,
            1_600,
            state,
            closed_at,
        )?;
        assert_eq!(restored.state(), state);
    }
    ProvisioningAuthorization::rehydrate(
        id(1)?,
        id(2)?,
        ProvisioningPurpose::Bootstrap,
        &token,
        1_000,
        1_600,
        ProvisioningState::Pending,
        None,
    )?;

    for (state, closed_at) in [
        (ProvisioningState::Pending, Some(1_200)),
        (ProvisioningState::Consumed, None),
        (ProvisioningState::Revoked, None),
        (ProvisioningState::Expired, None),
        (ProvisioningState::Revoked, Some(999)),
        (ProvisioningState::Expired, Some(1_599)),
    ] {
        assert_eq!(
            ProvisioningAuthorization::rehydrate(
                id(1)?,
                id(2)?,
                ProvisioningPurpose::Bootstrap,
                &token,
                1_000,
                1_600,
                state,
                closed_at
            ),
            Err(ProvisioningError::IncoherentState),
            "estado {state:?} cierre {closed_at:?}"
        );
    }
    Ok(())
}

#[test]
fn a_fresh_authorization_can_be_issued_after_the_previous_one_expires() -> TestResult {
    let first_token = digest('a');
    let second_token = digest('b');
    let mut first = authorization(&first_token)?;
    assert!(first.expire(1_600));

    let second = ProvisioningAuthorization::issue(
        id(2)?,
        id(3)?,
        ProvisioningPurpose::Bootstrap,
        &second_token,
        1_700,
        2_300,
    )?;
    assert_eq!(second.state(), ProvisioningState::Pending);
    assert!(second.is_live_at(1_800));
    assert!(!first.is_live_at(1_800));
    Ok(())
}

#[test]
fn purposes_and_states_round_trip_and_reject_unknown_values() -> TestResult {
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
    for state in [
        ProvisioningState::Pending,
        ProvisioningState::Consumed,
        ProvisioningState::Revoked,
        ProvisioningState::Expired,
    ] {
        assert_eq!(ProvisioningState::parse(state.as_str()), Ok(state));
    }
    assert_eq!(
        ProvisioningState::parse("DELETED"),
        Err(ProvisioningError::IncoherentState)
    );
    Ok(())
}
