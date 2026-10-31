// Fixtures de laboratorio para credenciales; no representan datos de producto.
#![allow(clippy::panic)]

use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use pos_core::{
    identity::{
        CredentialAlgorithm, CredentialKind, CredentialState, IdentityError, LockoutPolicy,
        UtcSeconds,
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
    credential::{
        CredentialAttempt, CredentialAuditEntry, CredentialAuditWriter, CredentialTransition,
        SqliteCredentialLab, VerificationGate, authenticate_and_upgrade as orchestrator, prepare,
    },
    identity::ensure_lab_identity_schema,
};

const ORG: &str = "00000000-0000-0000-0000-000000000080";
const BRANCH: &str = "00000000-0000-0000-0000-000000000090";
const USER: &str = "00000000-0000-0000-0000-0000000000a0";
const USER_SECOND: &str = "00000000-0000-0000-0000-0000000000a2";
const CREDENTIAL: &str = "00000000-0000-0000-0000-0000000000e0";
const CREDENTIAL_SECOND: &str = "00000000-0000-0000-0000-0000000000e2";
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
fn attempt_for<'a>(user: &str, password: &'a str, now: i64) -> CredentialAttempt<'a> {
    CredentialAttempt {
        organization_id: id(ORG),
        user_id: id(user),
        kind: CredentialKind::Password,
        password,
        now: moment(now),
        target_policy: Argon2Policy::LAB_V1,
        lockout: lockout(),
    }
}
fn attempt(password: &str, now: i64) -> CredentialAttempt<'_> {
    attempt_for(USER, password, now)
}
fn gate() -> VerificationGate {
    VerificationGate::new(4).unwrap_or_else(|error| panic!("gate: {error}"))
}
async fn authenticate_and_upgrade<W: CredentialAuditWriter>(
    pool: &SqlitePool,
    unit_of_work: &SqliteUnitOfWork,
    attempt: CredentialAttempt<'_>,
    audit: &W,
) -> Result<CredentialTransition, IdentityError> {
    orchestrator(pool, unit_of_work, attempt, audit, &gate()).await
}

/// Writer de auditoría de fixture; escribe en la misma conexión transaccional.
#[derive(Clone)]
struct FixtureAudit {
    fail: bool,
}
impl FixtureAudit {
    fn new(fail: bool) -> Self {
        Self { fail }
    }
}
impl CredentialAuditWriter for FixtureAudit {
    async fn append(
        &self,
        transaction: &mut SqliteTransaction,
        entry: &CredentialAuditEntry,
    ) -> Result<(), IdentityError> {
        if self.fail {
            return Err(IdentityError::StorageUnavailable);
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        sqlx::query(
            "INSERT INTO audit_event_fixture (id, organization_id, action) VALUES (?, ?, 'credential.changed')",
        )
        .bind(format!("audit-{id}"))
        .bind(entry.organization_id.as_str())
        .execute(&mut *transaction.connection())
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        Ok(())
    }
}

async fn open_lab(path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = open(path, Duration::from_millis(500)).await?;
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
            action TEXT NOT NULL\
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

async fn stored(
    pool: &SqlitePool,
) -> Result<pos_core::identity::LocalCredential, Box<dyn std::error::Error>> {
    let mut connection = pool.acquire().await?;
    let credential = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    Ok(credential)
}

#[tokio::test]
async fn legacy_bcrypt_login_upgrades_to_argon2id_in_one_commit()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-upgrade.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let audit = FixtureAudit::new(false);

    let outcome =
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await?;
    assert_eq!(
        outcome,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );
    let stored = stored(&pool).await?;
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
            .fetch_one(&pool)
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
    let audit = FixtureAudit::new(false);

    for (index, now) in [10, 11, 12].into_iter().enumerate() {
        let outcome =
            authenticate_and_upgrade(&pool, &unit_of_work, attempt("wrong-secret", now), &audit)
                .await?;
        if index < 2 {
            assert_eq!(outcome, CredentialTransition::Rejected);
        } else {
            assert_eq!(outcome, CredentialTransition::Locked);
        }
    }

    let locked = stored(&pool).await?;
    assert_eq!(locked.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(locked.failed_login_attempts(), 3);
    assert_eq!(locked.locked_until().map(UtcSeconds::get), Some(72));
    assert_eq!(locked.credential_version(), 4);

    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 50), &audit).await?,
        CredentialTransition::Locked
    );
    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 73), &audit).await?,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&pool)
            .await?,
        4
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_upgrade_cannot_reuse_a_stale_crypto_decision()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-stale.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let audit = FixtureAudit::new(false);

    let mut connection = pool.acquire().await?;
    let snapshot = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    drop(connection);
    let prepared = prepare(
        &snapshot,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;

    // Otra operación confirma el upgrade antes de aplicar la decisión preparada.
    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await?,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );

    let mut transaction = unit_of_work.begin().await?;
    let applied = SqliteCredentialLab::new(transaction.connection())
        .apply(
            &id(ORG),
            &id(USER),
            CredentialKind::Password,
            prepared,
            moment(21),
            lockout(),
        )
        .await;
    assert_eq!(applied, Err(IdentityError::ConcurrentModification));
    transaction.rollback().await?;

    let current = stored(&pool).await?;
    assert_eq!(current.algorithm(), CredentialAlgorithm::Argon2id);
    assert_eq!(current.credential_version(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&pool)
            .await?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn audit_failure_rolls_back_the_credential_upgrade() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-audit.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let audit = FixtureAudit::new(true);

    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await,
        Err(IdentityError::StorageUnavailable)
    );
    let current = stored(&pool).await?;
    assert_eq!(current.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(current.credential_version(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn corrupt_hash_requires_reset_without_fallback_or_further_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-reset.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", "not-a-hash", "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let audit = FixtureAudit::new(false);

    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await?,
        CredentialTransition::ResetRequired
    );
    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 21), &audit).await?,
        CredentialTransition::ResetRequired
    );

    let current = stored(&pool).await?;
    assert_eq!(current.state(), CredentialState::ResetRequired);
    assert_eq!(current.credential_version(), 2);
    assert_eq!(current.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&pool)
            .await?,
        1,
        "solo el cambio realmente persistido se audita"
    );
    Ok(())
}

#[tokio::test]
async fn disabled_and_locked_credentials_reject_before_expensive_verification()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-state.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "DISABLED", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let audit = FixtureAudit::new(false);

    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await?,
        CredentialTransition::Rejected
    );
    assert_eq!(stored(&pool).await?.credential_version(), 1);

    sqlx::query(
        "UPDATE identity_credential_lab SET state = 'ACTIVE', locked_until = 100 WHERE id = ?",
    )
    .bind(CREDENTIAL)
    .execute(&pool)
    .await?;
    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 50), &audit).await?,
        CredentialTransition::Locked
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn deactivated_user_between_snapshot_and_commit_is_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-user.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut connection = pool.acquire().await?;
    let snapshot = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    drop(connection);
    let prepared = prepare(
        &snapshot,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;
    sqlx::query("UPDATE identity_user_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(USER)
        .execute(&pool)
        .await?;

    let mut transaction = unit_of_work.begin().await?;
    let applied = SqliteCredentialLab::new(transaction.connection())
        .apply(
            &id(ORG),
            &id(USER),
            CredentialKind::Password,
            prepared,
            moment(20),
            lockout(),
        )
        .await?;
    transaction.commit().await?;
    assert_eq!(
        applied,
        crate::credential::AppliedCredential {
            transition: CredentialTransition::Rejected,
            persisted: false,
        }
    );
    let current = stored(&pool).await?;
    assert_eq!(current.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(current.credential_version(), 1);
    Ok(())
}

#[tokio::test]
async fn simultaneous_wrong_attempts_do_not_lose_updates() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("credential-concurrent-fail.sqlite");
    let first = open_lab(&path).await?;
    let second = open(&path, Duration::from_millis(500)).await?;
    seed_credential(&first, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let first_uow = SqliteUnitOfWork::new(first.clone());
    let second_uow = SqliteUnitOfWork::new(second.clone());
    let first_audit = FixtureAudit::new(false);
    let second_audit = FixtureAudit::new(false);

    let (left, right) = tokio::join!(
        authenticate_and_upgrade(
            &first,
            &first_uow,
            attempt("wrong-secret", 10),
            &first_audit
        ),
        authenticate_and_upgrade(
            &second,
            &second_uow,
            attempt("wrong-secret", 11),
            &second_audit
        )
    );
    assert_eq!(left?, CredentialTransition::Rejected);
    assert_eq!(right?, CredentialTransition::Rejected);
    let current = stored(&first).await?;
    assert_eq!(current.failed_login_attempts(), 2);
    assert_eq!(current.credential_version(), 3);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&first)
            .await?,
        2
    );
    Ok(())
}

#[tokio::test]
async fn simultaneous_upgrades_leave_one_winner_and_one_conflict()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("credential-concurrent-upgrade.sqlite");
    let first = open_lab(&path).await?;
    let second = open(&path, Duration::from_millis(500)).await?;
    seed_credential(&first, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let first_uow = SqliteUnitOfWork::new(first.clone());
    let second_uow = SqliteUnitOfWork::new(second.clone());
    let first_audit = FixtureAudit::new(false);
    let second_audit = FixtureAudit::new(false);

    let (left, right) = tokio::join!(
        authenticate_and_upgrade(&first, &first_uow, attempt(PASSWORD, 20), &first_audit),
        authenticate_and_upgrade(&second, &second_uow, attempt(PASSWORD, 21), &second_audit)
    );
    let outcomes = [left, right];
    let upgraded = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome,
                Ok(CredentialTransition::Verified {
                    rehash_performed: true
                })
            )
        })
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Err(IdentityError::ConcurrentModification)))
        .count();
    assert_eq!(upgraded, 1);
    assert_eq!(conflicts, 1);
    let current = stored(&first).await?;
    assert_eq!(current.algorithm(), CredentialAlgorithm::Argon2id);
    assert_eq!(current.credential_version(), 2);
    Ok(())
}

#[tokio::test]
async fn independent_writer_proceeds_while_argon2_verifies()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("credential-writer.sqlite");
    let first = open_lab(&path).await?;
    let second = open(&path, Duration::from_millis(500)).await?;
    seed_credential(&first, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let first_uow = SqliteUnitOfWork::new(first.clone());
    let audit = FixtureAudit::new(false);

    let task = tokio::spawn({
        let pool = first.clone();
        let unit_of_work = first_uow.clone();
        let audit = audit.clone();
        async move {
            authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await
        }
    });
    // Argon2id tarda cientos de milisegundos; durante ese tiempo no hay lock de
    // escritura retenido por la transacción de credenciales.
    tokio::time::sleep(Duration::from_millis(50)).await;
    sqlx::query(
        "INSERT INTO audit_event_fixture (id, organization_id, action) VALUES ('independent', ?, 'independent.write')",
    )
    .bind(ORG)
    .execute(&second)
    .await?;
    assert_eq!(
        task.await?,
        Ok(CredentialTransition::Verified {
            rehash_performed: true
        })
    );
    Ok(())
}

#[tokio::test]
async fn cancelled_task_leaves_credentials_untouched_and_pool_usable()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-cancel.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let audit = FixtureAudit::new(false);

    let handle = tokio::spawn({
        let pool = pool.clone();
        let unit_of_work = unit_of_work.clone();
        let audit = audit.clone();
        async move {
            authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await
        }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    handle.abort();
    let _ = handle.await;

    let current = stored(&pool).await?;
    assert_eq!(current.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(current.credential_version(), 1);
    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 21), &audit).await?,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );
    Ok(())
}

#[tokio::test]
async fn transaction_scope_excludes_crypto_duration() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-scope.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut connection = pool.acquire().await?;
    let snapshot = SqliteCredentialLab::new(&mut connection)
        .load(&id(ORG), &id(USER), CredentialKind::Password)
        .await?
        .unwrap_or_else(|| panic!("credential"));
    drop(connection);

    let crypto_start = std::time::Instant::now();
    let prepared = prepare(
        &snapshot,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;
    let crypto = crypto_start.elapsed();

    let transaction_start = std::time::Instant::now();
    let mut transaction = unit_of_work.begin().await?;
    let applied = SqliteCredentialLab::new(transaction.connection())
        .apply(
            &id(ORG),
            &id(USER),
            CredentialKind::Password,
            prepared,
            moment(20),
            lockout(),
        )
        .await?;
    transaction.commit().await?;
    let transaction_time = transaction_start.elapsed();

    assert!(applied.persisted);
    assert!(
        transaction_time < crypto,
        "tx={transaction_time:?} crypto={crypto:?}"
    );
    eprintln!(
        "crypto_ms={} tx_ms={}",
        crypto.as_millis(),
        transaction_time.as_millis()
    );
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

#[tokio::test]
async fn debug_output_never_contains_the_secret() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-debug.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;

    let attempt = attempt("super-secret-value", 10);
    let debug = format!("{attempt:?}");
    assert!(!debug.contains("super-secret-value"));
    assert!(debug.contains("<redacted>"));

    let snapshot = stored(&pool).await?;
    let prepared = prepare(
        &snapshot,
        CredentialKind::Password,
        "super-secret-value",
        Argon2Policy::LAB_V1,
    )?;
    assert_eq!(format!("{prepared:?}"), "PreparedCredential(REDACTED)");
    assert!(!format!("{snapshot:?}").contains(BCRYPT_HASH));
    Ok(())
}

#[tokio::test]
async fn rejected_attempt_after_rotation_is_not_applied() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-stale-reject.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let audit = FixtureAudit::new(false);

    let snapshot = stored(&pool).await?;
    let prepared = prepare(
        &snapshot,
        CredentialKind::Password,
        "wrong-secret",
        Argon2Policy::LAB_V1,
    )?;
    assert_eq!(
        authenticate_and_upgrade(&pool, &unit_of_work, attempt(PASSWORD, 20), &audit).await?,
        CredentialTransition::Verified {
            rehash_performed: true
        }
    );

    let mut transaction = unit_of_work.begin().await?;
    let applied = SqliteCredentialLab::new(transaction.connection())
        .apply(
            &id(ORG),
            &id(USER),
            CredentialKind::Password,
            prepared,
            moment(21),
            lockout(),
        )
        .await;
    assert_eq!(applied, Err(IdentityError::ConcurrentModification));
    transaction.rollback().await?;

    let current = stored(&pool).await?;
    assert_eq!(current.credential_version(), 2);
    assert_eq!(current.failed_login_attempts(), 0);
    Ok(())
}

#[tokio::test]
async fn reset_and_disable_between_snapshot_and_apply_are_observed()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("credential-stale-state.sqlite")).await?;
    seed_credential(&pool, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let snapshot = stored(&pool).await?;
    let prepared = prepare(
        &snapshot,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;
    sqlx::query("UPDATE identity_credential_lab SET state = 'RESET_REQUIRED' WHERE id = ?")
        .bind(CREDENTIAL)
        .execute(&pool)
        .await?;
    let mut transaction = unit_of_work.begin().await?;
    let applied = SqliteCredentialLab::new(transaction.connection())
        .apply(
            &id(ORG),
            &id(USER),
            CredentialKind::Password,
            prepared,
            moment(20),
            lockout(),
        )
        .await?;
    transaction.commit().await?;
    assert_eq!(
        applied,
        crate::credential::AppliedCredential {
            transition: CredentialTransition::ResetRequired,
            persisted: false,
        }
    );

    sqlx::query(
        "UPDATE identity_credential_lab SET state = 'ACTIVE', credential_version = 1 WHERE id = ?",
    )
    .bind(CREDENTIAL)
    .execute(&pool)
    .await?;
    let snapshot = stored(&pool).await?;
    let prepared = prepare(
        &snapshot,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;
    sqlx::query("UPDATE identity_credential_lab SET state = 'DISABLED' WHERE id = ?")
        .bind(CREDENTIAL)
        .execute(&pool)
        .await?;
    let mut transaction = unit_of_work.begin().await?;
    let applied = SqliteCredentialLab::new(transaction.connection())
        .apply(
            &id(ORG),
            &id(USER),
            CredentialKind::Password,
            prepared,
            moment(21),
            lockout(),
        )
        .await?;
    transaction.commit().await?;
    assert_eq!(
        applied,
        crate::credential::AppliedCredential {
            transition: CredentialTransition::Rejected,
            persisted: false,
        }
    );
    assert_eq!(stored(&pool).await?.credential_version(), 1);
    Ok(())
}

#[tokio::test]
async fn gate_limits_concurrent_verifications() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("credential-gate.sqlite");
    let first = open_lab(&path).await?;
    let second = open(&path, Duration::from_millis(500)).await?;
    seed_credential(&first, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    sqlx::query(
        "INSERT INTO identity_user_lab (id, organization_id, email, display_name, status, version) \
         VALUES (?, ?, 'segunda@lab.test', 'Segunda Operadora', 'ACTIVE', 0)",
    )
    .bind(USER_SECOND)
    .bind(ORG)
    .execute(&first)
    .await?;
    sqlx::query(
        "INSERT INTO identity_credential_lab (\
            id, user_id, organization_id, credential_kind, algorithm, secret_hash, params_json, \
            policy_id, state, credential_version, failed_login_attempts, locked_until, created_at, updated_at\
         ) VALUES (?, ?, ?, 'PASSWORD', 'LEGACY_BCRYPT', ?, '{\"cost\":4}', 'legacy-import-v1', 'ACTIVE', 1, 0, NULL, 10, 10)",
    )
    .bind(CREDENTIAL_SECOND)
    .bind(USER_SECOND)
    .bind(ORG)
    .bind(BCRYPT_HASH)
    .execute(&first)
    .await?;
    let first_uow = SqliteUnitOfWork::new(first.clone());
    let second_uow = SqliteUnitOfWork::new(second.clone());
    let audit = FixtureAudit::new(false);
    let limited = VerificationGate::new(1).unwrap_or_else(|error| panic!("gate: {error}"));

    let slow = tokio::spawn({
        let pool = first.clone();
        let unit_of_work = first_uow.clone();
        let audit = audit.clone();
        let limited = limited.clone();
        async move {
            orchestrator(
                &pool,
                &unit_of_work,
                attempt(PASSWORD, 20),
                &audit,
                &limited,
            )
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(limited.available_permits(), 0);

    let waiting = tokio::spawn({
        let pool = second.clone();
        let unit_of_work = second_uow.clone();
        let audit = audit.clone();
        let limited = limited.clone();
        async move {
            orchestrator(
                &pool,
                &unit_of_work,
                attempt_for(USER_SECOND, "wrong-secret", 21),
                &audit,
                &limited,
            )
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(!waiting.is_finished(), "la verificación excedió el límite");

    assert_eq!(
        slow.await?,
        Ok(CredentialTransition::Verified {
            rehash_performed: true
        })
    );
    assert_eq!(waiting.await?, Ok(CredentialTransition::Rejected));
    assert_eq!(limited.available_permits(), 1);
    let current = stored(&first).await?;
    assert_eq!(current.credential_version(), 2);
    assert_eq!(current.failed_login_attempts(), 0);
    let second_attempts = sqlx::query_scalar::<_, i64>(
        "SELECT failed_login_attempts FROM identity_credential_lab WHERE id = ?",
    )
    .bind(CREDENTIAL_SECOND)
    .fetch_one(&first)
    .await?;
    assert_eq!(second_attempts, 1);
    Ok(())
}

#[tokio::test]
async fn cancelled_while_waiting_for_capacity_leaves_state_consistent()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("credential-gate-cancel.sqlite");
    let first = open_lab(&path).await?;
    let second = open(&path, Duration::from_millis(500)).await?;
    seed_credential(&first, "LEGACY_BCRYPT", BCRYPT_HASH, "ACTIVE", None).await?;
    let first_uow = SqliteUnitOfWork::new(first.clone());
    let second_uow = SqliteUnitOfWork::new(second.clone());
    let audit = FixtureAudit::new(false);
    let limited = VerificationGate::new(1).unwrap_or_else(|error| panic!("gate: {error}"));

    let slow = tokio::spawn({
        let pool = first.clone();
        let unit_of_work = first_uow.clone();
        let audit = audit.clone();
        let limited = limited.clone();
        async move {
            orchestrator(
                &pool,
                &unit_of_work,
                attempt(PASSWORD, 20),
                &audit,
                &limited,
            )
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    let waiting = tokio::spawn({
        let pool = second.clone();
        let unit_of_work = second_uow.clone();
        let audit = audit.clone();
        let limited = limited.clone();
        async move {
            orchestrator(
                &pool,
                &unit_of_work,
                attempt("wrong-secret-cancelled", 21),
                &audit,
                &limited,
            )
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    waiting.abort();
    let _ = waiting.await;

    assert_eq!(
        slow.await?,
        Ok(CredentialTransition::Verified {
            rehash_performed: true
        })
    );
    let current = stored(&first).await?;
    assert_eq!(current.credential_version(), 2);
    assert_eq!(current.failed_login_attempts(), 0);
    assert_eq!(limited.available_permits(), 1);
    Ok(())
}
