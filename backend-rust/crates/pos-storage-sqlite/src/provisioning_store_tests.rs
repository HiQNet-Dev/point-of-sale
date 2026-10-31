#![allow(clippy::panic)]

//! Pruebas del adaptador de provisioning sobre la migración productiva `0017`.

use std::{path::Path, time::Duration};

use pos_core::{
    identity::{ProvisioningAuthorization, ProvisioningPurpose, ProvisioningState},
    organization_branch::EntityId,
};
use pos_credentials::ProvisioningToken;

use super::*;
use crate::provisioning::{ProvisioningStorageError, SqliteProvisioningStore};

const PROVISIONAL: &str = "00000000-0000-0000-0000-0000000002b0";

fn id(value: u128) -> EntityId {
    EntityId::parse(&format!("00000000-0000-0000-0000-{value:012x}"))
        .unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn provisional() -> EntityId {
    EntityId::parse(PROVISIONAL).unwrap_or_else(|error| panic!("provisional UUID: {error}"))
}

async fn open_candidate(path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = open(path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    sqlx::query(
        "INSERT INTO provisional_installation (id, fingerprint, created_at) \
         VALUES (?, 'ed25519:test-provisional', 900)",
    )
    .bind(PROVISIONAL)
    .execute(&pool)
    .await?;
    Ok(pool)
}

fn authorization(
    value: u128,
    token: &ProvisioningToken,
    issued_at: i64,
    expires_at: i64,
) -> ProvisioningAuthorization {
    ProvisioningAuthorization::issue(
        id(value),
        provisional(),
        ProvisioningPurpose::Bootstrap,
        &token.digest().hex(),
        issued_at,
        expires_at,
    )
    .unwrap_or_else(|error| panic!("authorization: {error}"))
}

async fn event_count(pool: &SqlitePool, event_type: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM bootstrap_authorization_event WHERE event_type = ?")
        .bind(event_type)
        .fetch_one(pool)
        .await
}

#[tokio::test]
async fn issue_and_read_round_trip_without_plaintext_token()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("provisioning.sqlite")).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;
    let authorization = authorization(1, &token, 1_000, 1_600);

    store.issue(&authorization, &id(900), 1_000).await?;

    let stored = store
        .get(&id(1))
        .await?
        .ok_or("la autorización emitida debe existir")?;
    assert_eq!(stored.state(), ProvisioningState::Pending);
    assert_eq!(stored.token_digest(), token.digest().hex());
    assert!(token.digest().matches(&token));
    assert_eq!(event_count(&pool, "ISSUED").await?, 1);

    // El token en claro no aparece en ninguna columna.
    let row: (String, String) =
        sqlx::query_as("SELECT token_digest, purpose FROM bootstrap_authorization WHERE id = ?")
            .bind(id(1).as_str())
            .fetch_one(&pool)
            .await?;
    assert!(!row.0.contains(token.expose()));
    assert_eq!(row.1, "BOOTSTRAP");
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn duplicate_digest_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("duplicate.sqlite")).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;

    store
        .issue(&authorization(1, &token, 1_000, 1_600), &id(900), 1_000)
        .await?;
    assert_eq!(
        store
            .issue(&authorization(3, &token, 1_000, 1_600), &id(901), 1_000)
            .await,
        Err(ProvisioningStorageError::AlreadyLive)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn a_live_right_blocks_reissue_but_an_expired_one_does_not()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("reissue.sqlite")).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let first = ProvisioningToken::generate()?;
    let second = ProvisioningToken::generate()?;

    store
        .issue(&authorization(1, &first, 1_000, 1_600), &id(900), 1_000)
        .await?;
    assert_eq!(
        store
            .issue(&authorization(2, &second, 1_300, 1_900), &id(901), 1_300)
            .await,
        Err(ProvisioningStorageError::AlreadyLive)
    );

    // Al consultar después de la caducidad, se materializa y se puede reemitir.
    store
        .issue(&authorization(3, &second, 1_700, 2_300), &id(902), 1_700)
        .await?;
    assert_eq!(
        store.get(&id(1)).await?.map(|value| value.state()),
        Some(ProvisioningState::Expired)
    );
    assert_eq!(
        store.get(&id(3)).await?.map(|value| value.state()),
        Some(ProvisioningState::Pending)
    );
    assert_eq!(event_count(&pool, "EXPIRED").await?, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn revoke_is_idempotent_and_audits_once() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("revoke.sqlite")).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;

    store
        .issue(&authorization(1, &token, 1_000, 1_600), &id(900), 1_000)
        .await?;
    assert_eq!(
        store.revoke(&id(1), &id(901), 1_100).await?,
        ProvisioningState::Revoked
    );
    assert_eq!(
        store.revoke(&id(1), &id(902), 1_150).await?,
        ProvisioningState::Revoked
    );
    assert_eq!(event_count(&pool, "REVOKED").await?, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn consume_is_single_use_and_rejects_a_wrong_token() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("consume.sqlite")).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;
    let other = ProvisioningToken::generate()?;
    store
        .issue(&authorization(1, &token, 1_000, 1_600), &id(900), 1_000)
        .await?;

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    let rejected = SqliteProvisioningStore::consume_in(
        &mut transaction,
        &id(1),
        &other.digest().hex(),
        &provisional(),
        ProvisioningPurpose::Bootstrap,
        &id(901),
        1_200,
    )
    .await;
    assert_eq!(
        rejected,
        Err(ProvisioningStorageError::Rejected(
            pos_core::identity::ProvisioningError::TokenMismatch
        ))
    );
    transaction.rollback().await?;
    assert_eq!(
        store.get(&id(1)).await?.map(|value| value.state()),
        Some(ProvisioningState::Pending)
    );

    let mut transaction = unit_of_work.begin().await?;
    SqliteProvisioningStore::consume_in(
        &mut transaction,
        &id(1),
        &token.digest().hex(),
        &provisional(),
        ProvisioningPurpose::Bootstrap,
        &id(902),
        1_200,
    )
    .await?;
    transaction.commit().await?;
    assert_eq!(
        store.get(&id(1)).await?.map(|value| value.state()),
        Some(ProvisioningState::Consumed)
    );

    let mut transaction = unit_of_work.begin().await?;
    assert_eq!(
        SqliteProvisioningStore::consume_in(
            &mut transaction,
            &id(1),
            &token.digest().hex(),
            &provisional(),
            ProvisioningPurpose::Bootstrap,
            &id(903),
            1_250,
        )
        .await,
        Err(ProvisioningStorageError::Rejected(
            pos_core::identity::ProvisioningError::AlreadyConsumed
        ))
    );
    transaction.rollback().await?;
    assert_eq!(event_count(&pool, "CONSUMED").await?, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn a_rolled_back_consume_leaves_no_trace() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("rollback.sqlite")).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;
    store
        .issue(&authorization(1, &token, 1_000, 1_600), &id(900), 1_000)
        .await?;

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    SqliteProvisioningStore::consume_in(
        &mut transaction,
        &id(1),
        &token.digest().hex(),
        &provisional(),
        ProvisioningPurpose::Bootstrap,
        &id(901),
        1_200,
    )
    .await?;
    transaction.rollback().await?;

    assert_eq!(
        store.get(&id(1)).await?.map(|value| value.state()),
        Some(ProvisioningState::Pending)
    );
    assert_eq!(event_count(&pool, "CONSUMED").await?, 0);
    assert_eq!(event_count(&pool, "ISSUED").await?, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn state_survives_reopen() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("reopen.sqlite");
    let pool = open_candidate(&path).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;
    store
        .issue(&authorization(1, &token, 1_000, 1_600), &id(900), 1_000)
        .await?;
    store.revoke(&id(1), &id(901), 1_100).await?;
    pool.close().await;

    let reopened = open(&path, Duration::from_millis(75)).await?;
    let store = SqliteProvisioningStore::new(reopened.clone());
    assert_eq!(
        store.get(&id(1)).await?.map(|value| value.state()),
        Some(ProvisioningState::Revoked)
    );
    reopened.close().await;
    Ok(())
}
