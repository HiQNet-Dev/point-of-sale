// Valida las propuestas físicas fuera de producto: sintaxis, CHECK, FKs,
// trigger append-only y durabilidad. No las convierte en migraciones.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_core::organization_branch::{
    BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewOrganization, Organization,
    Timezone,
};

use super::*;

const PROPOSAL_0002: &str =
    include_str!("../../../proposals/local-auth/0002_user_credential_session.proposed.sql");
const PROPOSAL_0003: &str =
    include_str!("../../../proposals/local-auth/0003_rbac_catalog.proposed.sql");
const PROPOSAL_0004: &str =
    include_str!("../../../proposals/local-auth/0004_branch_membership.proposed.sql");
const PROPOSAL_0005: &str =
    include_str!("../../../proposals/local-auth/0005_audit_event.proposed.sql");
const PROPOSAL_0006: &str =
    include_str!("../../../proposals/local-auth/0006_bootstrap_record.proposed.sql");

const ORG: &str = "00000000-0000-0000-0000-000000000080";
const ORG_TWO: &str = "00000000-0000-0000-0000-000000000081";
const USER: &str = "00000000-0000-0000-0000-0000000000a0";
const USER_TWO: &str = "00000000-0000-0000-0000-0000000000a1";
const SESSION: &str = "00000000-0000-0000-0000-0000000000d0";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const HASH: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

async fn apply_proposals(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for statement in [
        PROPOSAL_0002,
        PROPOSAL_0003,
        PROPOSAL_0004,
        PROPOSAL_0005,
        PROPOSAL_0006,
    ] {
        sqlx::raw_sql(statement).execute(pool).await?;
    }
    Ok(())
}

async fn seed_base(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    for (value, code) in [(ORG, "prop-org-a"), (ORG_TWO, "prop-org-b")] {
        let organization = Organization::create(
            id(value),
            NewOrganization {
                name: DisplayName::new("Propuesta").unwrap_or_else(|error| panic!("name: {error}")),
                code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
                timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("timezone: {error}")),
                currency: IsoCurrency::new("MXN")
                    .unwrap_or_else(|error| panic!("currency: {error}")),
                locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
            },
        );
        store.insert_organization(&organization, TIME, TIME).await?;
    }
    for (user, owner, email) in [
        (USER, ORG, "operadora@prop.test"),
        (USER_TWO, ORG_TWO, "otra@prop.test"),
    ] {
        sqlx::query(
            "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES (?, ?, 'Operadora', ?, 'ACTIVE', 0, ?, ?)",
        )
        .bind(user)
        .bind(owner)
        .bind(email)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO local_credential (id, user_id, organization_id, credential_kind, algorithm, secret_hash, params_json, policy_id, state, credential_version, failed_login_attempts, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000000e0', ?, ?, 'PASSWORD', 'LEGACY_BCRYPT', '$2b$04$synthetic', '{\"cost\":4}', 'legacy-import-v1', 'ACTIVE', 1, 0, ?, ?)",
    )
    .bind(USER)
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO local_session (id, user_id, session_hash, state, created_at, expires_at, revoked_at, session_version) \
         VALUES (?, ?, ?, 'ACTIVE', ?, ?, NULL, 1)",
    )
    .bind(SESSION)
    .bind(USER)
    .bind(HASH)
    .bind(TIME)
    .bind(LATER)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn proposed_schema_creates_checks_foreign_keys_and_append_only()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("proposals.sqlite");
    let pool = open(&path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    apply_proposals(&pool).await?;
    seed_base(&pool).await?;

    // D-AUTH-02: el catálogo físico no admite DISABLED.
    let disabled = sqlx::query("UPDATE local_credential SET state = 'DISABLED'")
        .execute(&pool)
        .await;
    assert!(disabled.is_err(), "DISABLED no debe ser un estado físico");
    let reset = sqlx::query("UPDATE local_credential SET state = 'PASSWORD_RESET_REQUIRED'")
        .execute(&pool)
        .await;
    assert!(reset.is_ok());
    sqlx::query("UPDATE local_credential SET state = 'ACTIVE'")
        .execute(&pool)
        .await?;

    // D-AUTH-04: credencial cross-organization rechazada por FK compuesta.
    let cross = sqlx::query(
        "INSERT INTO local_credential (id, user_id, organization_id, credential_kind, algorithm, secret_hash, params_json, policy_id, state, credential_version, failed_login_attempts, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000000e1', ?, ?, 'PIN', 'ARGON2ID', '$argon2id$x', '{\"m_cost\":1}', 'lab-argon2-v1', 'ACTIVE', 1, 0, ?, ?)",
    )
    .bind(USER)
    .bind(ORG_TWO)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await;
    assert!(cross.is_err(), "FK compuesta debe rechazar cross-org");

    // D-AUTH-07: actor USER/SYSTEM.
    let audit = |id_value: &'static str, actor: &'static str, user: Option<&'static str>| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO audit_event (id, organization_id, branch_id, actor_type, actor_user_id, session_id, action, entity_type, entity_id, reason, metadata_json, occurred_at, event_version) \
                 VALUES (?, ?, NULL, ?, ?, NULL, 'credential.updated', 'local_credential', NULL, NULL, NULL, ?, 1)",
            )
            .bind(id_value)
            .bind(ORG)
            .bind(actor)
            .bind(user)
            .bind(LATER)
            .execute(&pool)
            .await
        }
    };
    audit("00000000-0000-0000-0000-0000000000f0", "USER", Some(USER)).await?;
    audit("00000000-0000-0000-0000-0000000000f1", "SYSTEM", None).await?;
    assert!(
        audit("00000000-0000-0000-0000-0000000000f2", "SYSTEM", Some(USER))
            .await
            .is_err(),
        "SYSTEM con actor debe fallar"
    );
    assert!(
        audit("00000000-0000-0000-0000-0000000000f3", "USER", None)
            .await
            .is_err(),
        "USER sin actor debe fallar"
    );

    // D-AUTH-08: append-only.
    assert!(
        sqlx::query("UPDATE audit_event SET action = 'tampered'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM audit_event")
            .execute(&pool)
            .await
            .is_err()
    );

    // D-AUTH-10: singleton y estado completo.
    assert!(
        sqlx::query("INSERT INTO bootstrap_record (id) VALUES (2)")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE bootstrap_record SET completed_at = ? WHERE id = 1")
            .bind(TIME)
            .execute(&pool)
            .await
            .is_err(),
        "un estado parcial debe rechazarse"
    );

    // Durabilidad: reapertura ve los datos confirmados.
    pool.close().await;
    let reopened = open(&path, Duration::from_millis(75)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event")
            .fetch_one(&reopened)
            .await?,
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pos_user")
            .fetch_one(&reopened)
            .await?,
        2
    );
    reopened.close().await;
    Ok(())
}
