//! Pruebas del writer de auditoría contra el esquema candidato `audit_event`.
//!
//! Estas pruebas **no** promueven la migración: cargan el SQL candidato desde
//! `proposals/local-auth/` sobre una base temporal.
#![allow(clippy::panic)]

use std::{path::Path, time::Duration};

use pos_core::{
    audit::{
        AuditAction, AuditEntityType, AuditEventId, AuditEventRecord, AuditEventVersion,
        AuditMetadata, AuditOccurredAt,
    },
    identity::{LocalPrincipalService, SessionHash, UtcSeconds, VerifiedPrincipal},
    organization_branch::{
        BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewOrganization, Organization,
        Timezone,
    },
};
use serde_json::json;

use super::*;
use crate::{
    audit_writer::SqliteAuditWriter,
    identity_lab::{SqliteIdentityLab, ensure_lab_identity_schema},
};

const PROPOSAL_INSTALLATION: &str =
    include_str!("../../../proposals/local-auth/0002_installation_identity.proposed.sql");
const PROPOSAL_IDENTITY: &str =
    include_str!("../../../proposals/local-auth/0003_user_credential_session.proposed.sql");
const PROPOSAL_RBAC: &str =
    include_str!("../../../proposals/local-auth/0004_rbac_catalog.proposed.sql");
const PROPOSAL_MEMBERSHIP: &str =
    include_str!("../../../proposals/local-auth/0005_branch_membership.proposed.sql");
const PROPOSAL_AUDIT: &str =
    include_str!("../../../proposals/local-auth/0006_audit_event.proposed.sql");
const PROPOSAL_BOOTSTRAP: &str =
    include_str!("../../../proposals/local-auth/0007_bootstrap_record.proposed.sql");

const ORG: &str = "00000000-0000-0000-0000-0000000000a0";
const INSTALLATION: &str = "00000000-0000-0000-0000-0000000000f0";
const USER: &str = "00000000-0000-0000-0000-0000000000a1";
const SESSION: &str = "00000000-0000-0000-0000-0000000000d0";
const EVENT: &str = "00000000-0000-0000-0000-0000000000e0";
const EVENT_TWO: &str = "00000000-0000-0000-0000-0000000000e1";
const TIME: &str = "2026-10-29T00:00:00Z";
const HASH: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn moment(value: &str) -> AuditOccurredAt {
    AuditOccurredAt::new(value).unwrap_or_else(|error| panic!("fixture instante: {error}"))
}

async fn open_candidate(path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = open(path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    for statement in [
        PROPOSAL_INSTALLATION,
        PROPOSAL_IDENTITY,
        PROPOSAL_RBAC,
        PROPOSAL_MEMBERSHIP,
        PROPOSAL_AUDIT,
        PROPOSAL_BOOTSTRAP,
    ] {
        sqlx::raw_sql(statement).execute(&pool).await?;
    }
    Ok(pool)
}

async fn seed_org(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let organization = Organization::create(
        id(ORG),
        NewOrganization {
            name: DisplayName::new("Organización Audit")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("audit-org").unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    );
    SqliteOrganizationBranchStore::new(pool.clone())
        .insert_organization(&organization, TIME, TIME)
        .await?;
    Ok(())
}

async fn seed_lab_principal(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    ensure_lab_identity_schema(pool).await?;
    // Filas canónicas: la auditoría candidata exige FK contra `pos_user` y
    // `local_session`, aunque el principal se construya con el adaptador de
    // laboratorio mientras el productivo no esté habilitado.
    sqlx::query(
        "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
         VALUES (?, ?, 'Auditora', 'auditora@lab.test', 'ACTIVE', 0, ?, ?)",
    )
    .bind(USER)
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO local_session (id, user_id, session_hash, state, created_at, expires_at, revoked_at, session_version) \
         VALUES (?, ?, ?, 'ACTIVE', ?, '2026-10-30T00:00:00Z', NULL, 1)",
    )
    .bind(SESSION)
    .bind(USER)
    .bind(HASH)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO identity_user_lab (id, organization_id, email, display_name, status, version) \
         VALUES (?, ?, 'auditora@lab.test', 'Auditora', 'ACTIVE', 0)",
    )
    .bind(USER)
    .bind(ORG)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO identity_session_lab (id, user_id, session_hash, created_at, expires_at, revoked_at, session_version) \
         VALUES (?, ?, ?, 100, 200, NULL, 1)",
    )
    .bind(SESSION)
    .bind(USER)
    .bind(HASH)
    .execute(pool)
    .await?;
    Ok(())
}

async fn authenticate(pool: &SqlitePool) -> Result<VerifiedPrincipal, Box<dyn std::error::Error>> {
    let mut connection = pool.acquire().await?;
    let hash = SessionHash::new(HASH).unwrap_or_else(|error| panic!("hash: {error}"));
    let now = UtcSeconds::new(150).unwrap_or_else(|error| panic!("now: {error}"));
    let principal = LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
        .authenticate(&hash, now, None)
        .await?;
    Ok(principal)
}

fn bootstrap_event(value: &str) -> AuditEventRecord {
    AuditEventRecord::bootstrap_completed(
        AuditEventId::new(id(value)),
        id(ORG),
        id(INSTALLATION),
        moment(TIME),
    )
    .unwrap_or_else(|error| panic!("evento: {error}"))
}

#[tokio::test]
async fn system_bootstrap_event_is_persisted_and_survives_reopen()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("audit-writer.sqlite");
    let pool = open_candidate(&path).await?;
    seed_org(&pool).await?;

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    SqliteAuditWriter::append(&mut transaction, &bootstrap_event(EVENT)).await?;
    transaction.commit().await?;

    let row = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            String,
        ),
    >(
        "SELECT actor_type, actor_user_id, session_id, metadata_json, event_version, action \
         FROM audit_event WHERE id = ?",
    )
    .bind(EVENT)
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "SYSTEM");
    assert_eq!(row.1, None, "SYSTEM no lleva usuario");
    assert_eq!(row.2, None, "SYSTEM no lleva sesión");
    assert_eq!(row.3, None, "metadata vacía se persiste como NULL");
    assert_eq!(row.4, 1);
    assert_eq!(row.5, "bootstrap.completed");

    pool.close().await;
    let reopened = open(&path, Duration::from_millis(75)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event")
            .fetch_one(&reopened)
            .await?,
        1
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn audit_rows_are_append_only_after_reopen() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("audit-append-only.sqlite");
    let pool = open_candidate(&path).await?;
    seed_org(&pool).await?;

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    SqliteAuditWriter::append(&mut transaction, &bootstrap_event(EVENT)).await?;
    transaction.commit().await?;

    assert!(
        sqlx::query("UPDATE audit_event SET action = 'tampered'")
            .execute(&pool)
            .await
            .is_err(),
        "UPDATE debe rechazarse"
    );
    assert!(
        sqlx::query("DELETE FROM audit_event")
            .execute(&pool)
            .await
            .is_err(),
        "DELETE debe rechazarse"
    );
    pool.close().await;

    let reopened = open(&path, Duration::from_millis(75)).await?;
    assert!(
        sqlx::query("DELETE FROM audit_event")
            .execute(&reopened)
            .await
            .is_err(),
        "el trigger sobrevive a la reapertura"
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn failed_transaction_leaves_no_audit_row() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("audit-rollback.sqlite")).await?;
    seed_org(&pool).await?;

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    SqliteAuditWriter::append(&mut transaction, &bootstrap_event(EVENT)).await?;
    transaction.rollback().await?;

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event")
            .fetch_one(&pool)
            .await?,
        0,
        "la auditoría no sobrevive a un rollback"
    );
    Ok(())
}

#[tokio::test]
async fn writer_rejects_a_system_event_carrying_a_session() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("audit-attribution.sqlite")).await?;
    seed_org(&pool).await?;

    let mut event = bootstrap_event(EVENT);
    event.session_id = Some(id("00000000-0000-0000-0000-0000000000d9"));

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    assert!(
        SqliteAuditWriter::append(&mut transaction, &event)
            .await
            .is_err(),
        "un evento SYSTEM no puede llevar sesión"
    );
    transaction.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn user_event_from_a_verified_principal_records_actor_session_and_canonical_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("audit-user.sqlite")).await?;
    seed_org(&pool).await?;
    seed_lab_principal(&pool).await?;
    let principal = authenticate(&pool).await?;
    assert_eq!(principal.user_id(), &id(USER));

    let metadata = AuditMetadata::new(json!({
        "z": ["status"],
        "a": { "count": 2 }
    }))
    .unwrap_or_else(|error| panic!("metadata: {error}"));
    let event = AuditEventRecord::from_verified_principal(
        AuditEventId::new(id(EVENT_TWO)),
        &principal,
        AuditAction::new("organization.updated").unwrap_or_else(|error| panic!("acción: {error}")),
        AuditEntityType::new("organization").unwrap_or_else(|error| panic!("tipo: {error}")),
        Some(id(ORG)),
        metadata,
        moment("2026-10-29T14:35:12.123Z"),
    );

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    SqliteAuditWriter::append(&mut transaction, &event).await?;
    transaction.commit().await?;

    let row = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
        ),
    >(
        "SELECT actor_type, actor_user_id, session_id, metadata_json, occurred_at \
         FROM audit_event WHERE id = ?",
    )
    .bind(EVENT_TWO)
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "USER");
    assert_eq!(row.1.as_deref(), Some(USER));
    assert_eq!(
        row.2.as_deref(),
        Some(SESSION),
        "la sesión pertenece al actor"
    );
    assert_eq!(
        row.3.as_deref(),
        Some(r#"{"a":{"count":2},"z":["status"]}"#),
        "metadata canónica y determinista"
    );
    assert_eq!(
        row.4, "2026-10-29T14:35:12.123Z",
        "la fracción de segundo se conserva; el CHECK candidato la admite"
    );
    Ok(())
}

#[tokio::test]
async fn event_version_and_metadata_are_not_truncated() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join("audit-metadata.sqlite")).await?;
    seed_org(&pool).await?;

    let mut event = bootstrap_event(EVENT);
    event.metadata_json = AuditMetadata::new(json!({ "note": "x".repeat(1024) }))
        .unwrap_or_else(|error| panic!("metadata: {error}"));
    event.event_version =
        AuditEventVersion::new(2).unwrap_or_else(|error| panic!("versión: {error}"));

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    SqliteAuditWriter::append(&mut transaction, &event).await?;
    transaction.commit().await?;

    let (stored, version) = sqlx::query_as::<_, (String, i64)>(
        "SELECT metadata_json, event_version FROM audit_event WHERE id = ?",
    )
    .bind(EVENT)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored.len(), event.metadata_json.canonical_json().len());
    assert!(stored.contains("note"));
    assert_eq!(version, 2);
    Ok(())
}
