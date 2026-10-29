// Fixtures de laboratorio para credenciales; no representan datos de producto.
#![allow(clippy::panic)]

use std::{path::Path, time::Duration};

use pos_core::{
    identity::{
        CredentialAlgorithm, CredentialKind, CredentialState, LocalPrincipalService, LockoutPolicy,
        SessionHash, UtcSeconds,
    },
    organization_branch::{
        Branch, BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewBranch,
        NewOrganization, Organization, Timezone,
    },
};
use pos_credentials::{
    Argon2Policy, SessionHandle, VerificationOutcome, VerifyRequest, verify_credential,
};

use super::*;
use crate::{
    credential_lab::{CredentialAttempt, CredentialTransition, SqliteCredentialLab},
    identity_lab::{SqliteIdentityLab, ensure_lab_identity_schema},
};

const ORG: &str = "00000000-0000-0000-0000-000000000080";
const BRANCH: &str = "00000000-0000-0000-0000-000000000090";
const USER: &str = "00000000-0000-0000-0000-0000000000a0";
const CREDENTIAL: &str = "00000000-0000-0000-0000-0000000000e0";
const SESSION: &str = "00000000-0000-0000-0000-0000000000d0";
const TIME: &str = "2026-10-29T00:00:00Z";
/// Vector Spring `$2b$` público y sintético para `lab-only-Piñata-42!`.
const BCRYPT_HASH: &str = "$2b$04$WLGfreUeNXWqw6cV3Pq9IOaIeeH/v7O8PxPv/l.Ukazkf8D4nwh.6";
const PASSWORD: &str = "lab-only-Piñata-42!";
const SESSION_SECRET: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture time: {error}"))
}
fn lockout() -> LockoutPolicy {
    LockoutPolicy::new(3, 60).unwrap_or_else(|error| panic!("policy: {error}"))
}
fn attempt<'a>(password: &'a str, now: i64) -> CredentialAttempt<'a> {
    CredentialAttempt {
        organization_id: id(ORG),
        user_id: id(USER),
        kind: CredentialKind::Password,
        password,
        now: moment(now),
        target_policy: Argon2Policy::LAB_V1,
        lockout: lockout(),
    }
}

async fn open_lab(path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = open(path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    ensure_lab_identity_schema(&pool).await?;
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    let organization = Organization::create(
        id(ORG),
        NewOrganization {
            name: DisplayName::new("Organización Lab")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("lab-org").unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("timezone: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    );
    store.insert_organization(&organization, TIME, TIME).await?;
    let branch = Branch::create(
        id(BRANCH),
        organization.id().clone(),
        NewBranch {
            name: DisplayName::new("Sucursal Lab").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("lab-a1").unwrap_or_else(|error| panic!("code: {error}")),
            timezone_override: None,
        },
    );
    store.insert_branch(&branch, TIME, TIME).await?;
    sqlx::query(
        "INSERT INTO identity_user_lab (id, organization_id, email, display_name, status, version) \
         VALUES (?, ?, 'operadora@lab.test', 'Operadora Lab', 'ACTIVE', 0)",
    )
    .bind(USER)
    .bind(ORG)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO identity_session_lab (id, user_id, session_hash, created_at, expires_at, revoked_at, session_version) \
         VALUES (?, ?, ?, 10, 1000, NULL, 1)",
    )
    .bind(SESSION)
    .bind(USER)
    .bind(SESSION_SECRET)
    .execute(&pool)
    .await?;
    sqlx::query(
        "CREATE TABLE audit_event_fixture (\
            id TEXT PRIMARY KEY NOT NULL, \
            organization_id TEXT NOT NULL, \
            action TEXT NOT NULL CHECK (action <> 'forbidden')\
         ) STRICT",
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

async fn seed_credential(
    pool: &SqlitePool,
    algorithm: &str,
    secret_hash: &str,
    state: &str,
    locked_until: Option<i64>,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "INSERT INTO identity_credential_lab (\
            id, user_id, organization_id, credential_kind, algorithm, secret_hash, params_json, \
            policy_id, state, credential_version, failed_login_attempts, locked_until, created_at, updated_at\
         ) VALUES (?, ?, ?, 'PASSWORD', ?, ?, '{\"cost\":4}', 'legacy-import-v1', ?, 1, 0, ?, 10, 10)",
    )
    .bind(CREDENTIAL)
    .bind(USER)
    .bind(ORG)
    .bind(algorithm)
    .bind(secret_hash)
    .bind(state)
    .bind(locked_until)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn legacy_bcrypt_login_upgrades_to_argon2id_in_one_commit()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-upgrade.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut transaction = unit_of_work.begin().await?;
    let outcome = SqliteCredentialLab::new(transaction.connection())
        .verify_and_upgrade(attempt(PASSWORD, 20))
        .await?;
    assert_eq!(
        outcome,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );
    sqlx::query(
        "INSERT INTO audit_event_fixture (id, organization_id, action) VALUES (?, ?, 'credential.upgraded')",
    )
    .bind("00000000-0000-0000-0000-0000000000f0")
    .bind(ORG)
    .execute(&mut *transaction.connection())
    .await?;
    transaction.commit().await?;

    let mut connection = pool.acquire().await?;
    let stored = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    assert_eq!(stored.algorithm(), CredentialAlgorithm::Argon2id);
    assert_eq!(stored.credential_version(), 2);
    assert_eq!(stored.policy_id(), "lab-argon2-v1");
    assert_eq!(stored.failed_login_attempts(), 0);
    assert_eq!(
        verify_credential(&VerifyRequest {
            purpose: CredentialKind::Password,
            algorithm: stored.algorithm(),
            stored: stored.secret_hash(),
            password: PASSWORD,
        }),
        Ok(VerificationOutcome::Verified {
            rehash_required: false
        })
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&mut *connection)
            .await?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn wrong_password_locks_after_policy_and_never_upgrades()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-lock.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    for (attempt_index, now) in [10, 11, 12].into_iter().enumerate() {
        let mut transaction = unit_of_work.begin().await?;
        let outcome = SqliteCredentialLab::new(transaction.connection())
            .verify_and_upgrade(attempt("wrong-secret", now))
            .await?;
        transaction.commit().await?;
        if attempt_index < 2 {
            assert_eq!(outcome, CredentialTransition::Rejected);
        } else {
            assert_eq!(outcome, CredentialTransition::Locked);
        }
    }

    let mut connection = pool.acquire().await?;
    let locked = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    assert_eq!(locked.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(locked.failed_login_attempts(), 3);
    assert_eq!(locked.locked_until().map(UtcSeconds::get), Some(72));
    assert_eq!(locked.credential_version(), 4);

    let blocked = SqliteCredentialLab::new(&mut connection)
        .verify_and_upgrade(attempt(PASSWORD, 50))
        .await?;
    assert_eq!(blocked, CredentialTransition::Locked);

    // La ventana vencida permite verificar y migrar.
    let after_window = SqliteCredentialLab::new(&mut connection)
        .verify_and_upgrade(attempt(PASSWORD, 73))
        .await?;
    assert_eq!(
        after_window,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );
    Ok(())
}

#[tokio::test]
async fn cas_conflict_leaves_the_stored_material_untouched()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-cas.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut connection = pool.acquire().await?;
    let stale = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    drop(connection);

    let mut transaction = unit_of_work.begin().await?;
    SqliteCredentialLab::new(transaction.connection())
        .verify_and_upgrade(attempt(PASSWORD, 20))
        .await?;
    transaction.commit().await?;

    let mut connection = pool.acquire().await?;
    assert_eq!(
        SqliteCredentialLab::new(&mut connection)
            .persist(&id(ORG), 1, &stale)
            .await?,
        CasOutcome::Conflict
    );
    let current = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    assert_eq!(current.algorithm(), CredentialAlgorithm::Argon2id);
    assert_eq!(current.credential_version(), 2);
    Ok(())
}

#[tokio::test]
async fn audit_failure_rolls_back_the_credential_upgrade() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-audit.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut transaction = unit_of_work.begin().await?;
    let outcome = SqliteCredentialLab::new(transaction.connection())
        .verify_and_upgrade(attempt(PASSWORD, 20))
        .await?;
    assert_eq!(
        outcome,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );
    let audit = sqlx::query(
        "INSERT INTO audit_event_fixture (id, organization_id, action) VALUES (?, ?, 'forbidden')",
    )
    .bind("00000000-0000-0000-0000-0000000000f1")
    .bind(ORG)
    .execute(&mut *transaction.connection())
    .await;
    assert!(audit.is_err());
    transaction.rollback().await?;

    let mut connection = pool.acquire().await?;
    let stored = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    assert_eq!(stored.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(stored.credential_version(), 1);
    Ok(())
}

#[tokio::test]
async fn corrupt_hash_requires_reset_without_fallback_or_further_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-reset.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", "not-a-hash", "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    for now in [20, 21] {
        let mut transaction = unit_of_work.begin().await?;
        let outcome = SqliteCredentialLab::new(transaction.connection())
            .verify_and_upgrade(attempt(PASSWORD, now))
            .await?;
        transaction.commit().await?;
        assert_eq!(outcome, CredentialTransition::ResetRequired);
    }

    let mut connection = pool.acquire().await?;
    let stored = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    assert_eq!(stored.state(), CredentialState::ResetRequired);
    assert_eq!(stored.credential_version(), 2);
    assert_eq!(stored.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    Ok(())
}

#[tokio::test]
async fn disabled_and_locked_credentials_reject_before_expensive_verification()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-state.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "DISABLED", None).await?;
    let mut connection = pool.acquire().await?;
    assert_eq!(
        SqliteCredentialLab::new(&mut connection)
            .verify_and_upgrade(attempt(PASSWORD, 20))
            .await?,
        CredentialTransition::Rejected
    );
    let stored = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    assert_eq!(stored.state(), CredentialState::Disabled);
    assert_eq!(stored.credential_version(), 1);
    drop(connection);

    sqlx::query(
        "UPDATE identity_credential_lab SET state = 'ACTIVE', locked_until = 100 WHERE id = ?",
    )
    .bind(CREDENTIAL)
    .execute(&pool)
    .await?;
    let mut connection = pool.acquire().await?;
    assert_eq!(
        SqliteCredentialLab::new(&mut connection)
            .verify_and_upgrade(attempt(PASSWORD, 50))
            .await?,
        CredentialTransition::Locked
    );
    Ok(())
}

#[tokio::test]
async fn scoped_verification_only_touches_the_authenticated_principal()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-principal.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut transaction = unit_of_work.begin().await?;
    let principal = LocalPrincipalService::new(SqliteIdentityLab::new(transaction.connection()))
        .authenticate(
            &SessionHash::new(SESSION_SECRET).unwrap_or_else(|error| panic!("hash: {error}")),
            moment(11),
            None,
        )
        .await
        .unwrap_or_else(|error| panic!("principal: {error}"));
    transaction.rollback().await?;

    let mut transaction = unit_of_work.begin().await?;
    let outcome = SqliteCredentialLab::new(transaction.connection())
        .verify_and_upgrade_for(
            &principal,
            PASSWORD,
            moment(20),
            Argon2Policy::LAB_V1,
            lockout(),
        )
        .await?;
    assert_eq!(
        outcome,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );
    transaction.commit().await?;
    Ok(())
}

#[tokio::test]
async fn session_handle_stores_only_its_hash() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-session.sqlite")).await?;
    let handle = SessionHandle::generate().unwrap_or_else(|error| panic!("handle: {error}"));
    let stored = handle
        .hash()
        .unwrap_or_else(|error| panic!("hash: {error}"));
    sqlx::query(
        "INSERT INTO identity_session_lab (id, user_id, session_hash, created_at, expires_at, revoked_at, session_version) \
         VALUES ('00000000-0000-0000-0000-0000000000d1', ?, ?, 10, 1000, NULL, 1)",
    )
    .bind(USER)
    .bind(stored.as_storage_str())
    .execute(&pool)
    .await?;
    let persisted = sqlx::query_scalar::<_, String>(
        "SELECT session_hash FROM identity_session_lab WHERE id = '00000000-0000-0000-0000-0000000000d1'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(persisted, stored.as_storage_str());
    assert_ne!(persisted, handle.as_client_secret());
    Ok(())
}
