//! Integración real de autenticación: provisioning → login → sesión → logout.
//!
//! Todo el recorrido usa el esquema productivo (`0001`–`0017`), el migrador
//! aprobado, Argon2id real y SQLite en archivo. No hay fakes de persistencia ni
//! credenciales de bypass.
#![allow(clippy::panic)]

use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use pos_application::{
    bootstrap::{BootstrapRequest, ProvisioningProof, provision_initial_installation_with_proof},
    identity::{AuthenticatedSession, LoginRequest, MAX_SESSION_TTL_SECONDS},
};
use pos_core::{
    audit::{
        AuditEventId, AuditOccurredAt, BootstrapProvisioning, BootstrapSecretVerifier,
        authorize_bootstrap,
    },
    identity::{
        NormalizedEmail, ProvisioningAuthorization, ProvisioningPurpose, SessionHash, UtcSeconds,
    },
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        IsoCurrency, LocaleTag, NewOrganization, Organization, Status, Timezone,
    },
};
use pos_credentials::{Argon2Policy, ProvisioningToken};
use sqlx::SqlitePool;

use crate::{
    SqliteUnitOfWork, bootstrap::CandidateBootstrapPasswords, migrate, open,
    provisioning::SqliteProvisioningStore, session::identity_application,
};

const ORG: &str = "00000000-0000-0000-0000-00000000c0a0";
const ADMIN: &str = "00000000-0000-0000-0000-00000000c0a1";
const ROLE: &str = "00000000-0000-0000-0000-00000000c0b0";
const BRANCH: &str = "00000000-0000-0000-0000-00000000c090";
const INACTIVE_BRANCH: &str = "00000000-0000-0000-0000-00000000c091";
const FOREIGN_ORG: &str = "00000000-0000-0000-0000-00000000c0af";
const FOREIGN_BRANCH: &str = "00000000-0000-0000-0000-00000000c092";
const INSTALLATION: &str = "00000000-0000-0000-0000-00000000c0f0";
const AUTHORIZATION: &str = "00000000-0000-0000-0000-00000000c1a0";
const ISSUED_EVENT: &str = "00000000-0000-0000-0000-00000000c1e0";
const BOOTSTRAP_EVENT: &str = "00000000-0000-0000-0000-00000000c0e0";
const PASSWORD: &str = "contraseña-local-segura-2k";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn email() -> NormalizedEmail {
    NormalizedEmail::new("admin@hiqnet.test").unwrap_or_else(|error| panic!("email: {error}"))
}

fn now_epoch() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("reloj: {error}"));
    i64::try_from(elapsed.as_secs()).unwrap_or_else(|error| panic!("instante: {error}"))
}

async fn iso_now(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("iso: {error}"))
}

struct TestVerifier;
impl BootstrapSecretVerifier for TestVerifier {
    fn verify(&self, presented: &[u8]) -> Result<(), pos_core::audit::BootstrapError> {
        if presented == b"capability-de-prueba" {
            Ok(())
        } else {
            Err(pos_core::audit::BootstrapError::Rejected)
        }
    }
}

fn capability() -> BootstrapProvisioning {
    authorize_bootstrap(&TestVerifier, b"capability-de-prueba")
        .unwrap_or_else(|_| panic!("capacidad"))
}

/// Generador de identidades para eventos de auditoría.
struct Identities;
impl IdentityGenerator for Identities {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        let value = pos_credentials::random_uuid_v4().map_err(|_| ApplicationError::Storage)?;
        EntityId::parse(&value).map_err(|_| ApplicationError::Storage)
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    path: PathBuf,
    pool: SqlitePool,
}

async fn provisioned() -> Result<Fixture, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("auth.sqlite");
    let pool = open(&path, Duration::from_millis(500)).await?;
    migrate(&pool).await?;
    seed(&pool).await?;
    Ok(Fixture {
        _directory: directory,
        path,
        pool,
    })
}

fn branch(value: &str, organization: &str, code: &str, status: Status) -> Branch {
    Branch::rehydrate(
        id(value),
        id(organization),
        DisplayName::new("Sucursal").unwrap_or_else(|error| panic!("name: {error}")),
        BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
        None,
        status,
        0,
    )
}

async fn seed(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let now = now_epoch();
    let occurred_at = iso_now(pool).await;
    let installation = id(INSTALLATION);
    let store = SqliteProvisioningStore::new(pool.clone());
    store
        .ensure_provisional_installation(&installation, "local:auth-test", now)
        .await?;
    let token = ProvisioningToken::generate()?;
    let digest = token.digest().hex();
    let authorization = ProvisioningAuthorization::issue(
        id(AUTHORIZATION),
        installation.clone(),
        ProvisioningPurpose::Bootstrap,
        &digest,
        now,
        now + 600,
    )?;
    store.issue(&authorization, &id(ISSUED_EVENT), now).await?;

    let request = BootstrapRequest {
        installation_id: installation,
        audit_event_id: AuditEventId::new(id(BOOTSTRAP_EVENT)),
        organization_id: id(ORG),
        organization: NewOrganization {
            name: DisplayName::new("Organización Auth")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("auth-org").unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("moneda: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
        administrator_id: id(ADMIN),
        administrator_name: DisplayName::new("Administradora")
            .unwrap_or_else(|error| panic!("name: {error}")),
        administrator_email: email(),
        administrator_role_id: id(ROLE),
        administrator_role_code: BusinessCode::new("administrator")
            .unwrap_or_else(|error| panic!("rol: {error}")),
        installation_fingerprint: format!("local:{INSTALLATION}"),
        initial_credential: CandidateBootstrapPasswords::new(Argon2Policy::LAB_V1).prepare(
            id(ADMIN),
            id(ADMIN),
            PASSWORD,
            UtcSeconds::new(now).unwrap_or_else(|error| panic!("instante: {error}")),
        )?,
    };
    let proof = ProvisioningProof {
        authorization_id: &id(AUTHORIZATION),
        presented_digest: &digest,
        now_epoch: now,
    };
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    provision_initial_installation_with_proof(
        &mut transaction,
        &capability(),
        &request,
        &proof,
        &occurred_at,
    )
    .await?;
    transaction.commit().await?;

    let mut transaction = unit_of_work.begin().await?;
    transaction
        .insert_branch(&branch(BRANCH, ORG, "centro", Status::Active), &occurred_at)
        .await?;
    transaction
        .insert_branch(
            &branch(INACTIVE_BRANCH, ORG, "cerrada", Status::Inactive),
            &occurred_at,
        )
        .await?;
    transaction.commit().await?;

    let mut transaction = unit_of_work.begin().await?;
    transaction
        .insert_organization(
            &Organization::create(
                id(FOREIGN_ORG),
                NewOrganization {
                    name: DisplayName::new("Organización Ajena")
                        .unwrap_or_else(|error| panic!("name: {error}")),
                    code: BusinessCode::new("ajena")
                        .unwrap_or_else(|error| panic!("code: {error}")),
                    timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
                    currency: IsoCurrency::new("MXN")
                        .unwrap_or_else(|error| panic!("moneda: {error}")),
                    locale: LocaleTag::new("es-MX")
                        .unwrap_or_else(|error| panic!("locale: {error}")),
                },
            ),
            &occurred_at,
        )
        .await?;
    transaction
        .insert_branch(
            &branch(FOREIGN_BRANCH, FOREIGN_ORG, "ajena", Status::Active),
            &occurred_at,
        )
        .await?;
    transaction.commit().await?;
    Ok(())
}

fn request(ttl: i64) -> LoginRequest {
    LoginRequest {
        email: email(),
        now: UtcSeconds::new(now_epoch()).unwrap_or_else(|error| panic!("instante: {error}")),
        occurred_at: AuditOccurredAt::new("2026-10-31T00:00:00Z")
            .unwrap_or_else(|error| panic!("fecha: {error}")),
        session_ttl_seconds: ttl,
    }
}

async fn login(
    fixture: &Fixture,
    password: &str,
) -> Result<AuthenticatedSession, ApplicationError> {
    let application =
        identity_application(fixture.pool.clone()).unwrap_or_else(|error| panic!("app: {error}"));
    let mut identities = Identities;
    application
        .login(&mut identities, &request(3600), password)
        .await
}

fn hash_of(handle: &str) -> SessionHash {
    pos_credentials::SessionHandle::from_encoded(handle)
        .and_then(|value| value.hash())
        .unwrap_or_else(|error| panic!("handle: {error}"))
}

#[tokio::test]
async fn admin_logs_in_reads_identity_and_logs_out() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    let application = identity_application(fixture.pool.clone())?;
    let mut identities = Identities;
    let request = request(3600);
    let session = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    assert_eq!(session.identity.user_id, id(ADMIN));
    assert_eq!(session.identity.organization_id, id(ORG));
    assert_eq!(session.identity.email, email().as_str());
    assert!(session.identity.permissions.contains("branch.read"));
    assert!(session.identity.branch_id.is_none());

    let current = application
        .current(&hash_of(&session.handle), request.now)
        .await?;
    assert_eq!(current, session.identity);

    application
        .logout(
            &mut identities,
            &hash_of(&session.handle),
            request.now,
            &request.occurred_at,
        )
        .await?;
    assert_eq!(
        application
            .current(&hash_of(&session.handle), request.now)
            .await,
        Err(ApplicationError::Unauthenticated)
    );
    // Logout repetido no falla.
    application
        .logout(
            &mut identities,
            &hash_of(&session.handle),
            request.now,
            &request.occurred_at,
        )
        .await?;
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn login_persists_across_reopen_and_audits() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    let first = login(&fixture, PASSWORD).await?;
    assert!(first.identity.permissions.len() >= 59);
    fixture.pool.close().await;

    let reopened = open(&fixture.path, Duration::from_millis(500)).await?;
    let application = identity_application(reopened.clone())?;
    let mut identities = Identities;
    let request = request(3600);
    let second = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    assert_eq!(second.identity.user_id, first.identity.user_id);
    assert_eq!(second.identity.permissions, first.identity.permissions);

    let audits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_event WHERE action = 'auth.login'")
            .fetch_one(&reopened)
            .await?;
    assert_eq!(audits, 2);
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn wrong_password_unknown_user_inactive_and_locked_share_one_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    assert_eq!(
        login(&fixture, "incorrecta").await.err(),
        Some(ApplicationError::InvalidCredentials)
    );

    let application = identity_application(fixture.pool.clone())?;
    let mut identities = Identities;
    let mut unknown = request(3600);
    unknown.email = NormalizedEmail::new("nadie@hiqnet.test")?;
    assert_eq!(
        application
            .login(&mut identities, &unknown, PASSWORD)
            .await
            .err(),
        Some(ApplicationError::InvalidCredentials)
    );

    // Cuenta inactiva.
    sqlx::query("UPDATE pos_user SET status = 'INACTIVE' WHERE id = ?")
        .bind(ADMIN)
        .execute(&fixture.pool)
        .await?;
    assert_eq!(
        login(&fixture, PASSWORD).await.err(),
        Some(ApplicationError::InvalidCredentials)
    );
    sqlx::query("UPDATE pos_user SET status = 'ACTIVE' WHERE id = ?")
        .bind(ADMIN)
        .execute(&fixture.pool)
        .await?;

    // Bloqueo tras cinco intentos fallidos.
    for _ in 0..5 {
        let _ = login(&fixture, "incorrecta").await;
    }
    let attempts: i64 =
        sqlx::query_scalar("SELECT failed_login_attempts FROM local_credential WHERE user_id = ?")
            .bind(ADMIN)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(attempts, 5);
    assert_eq!(
        login(&fixture, PASSWORD).await.err(),
        Some(ApplicationError::InvalidCredentials)
    );

    let failures: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_event WHERE action = 'auth.login_failed'")
            .fetch_one(&fixture.pool)
            .await?;
    // Un evento por incremento real: el intento posterior al bloqueo no cuenta
    // ni se audita de nuevo.
    assert_eq!(failures, 5);
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_session")
        .fetch_one(&fixture.pool)
        .await?;
    assert_eq!(sessions, 0);
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn expired_revoked_and_replayed_sessions_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    let application = identity_application(fixture.pool.clone())?;
    let mut identities = Identities;
    let request = request(3600);

    let session = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    let hash = hash_of(&session.handle);
    sqlx::query(
        "UPDATE local_session \
         SET created_at = strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'), \
             expires_at = strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 hour') \
         WHERE session_hash = ?",
    )
    .bind(hash.as_storage_str())
    .execute(&fixture.pool)
    .await?;
    assert_eq!(
        application.current(&hash, request.now).await,
        Err(ApplicationError::Unauthenticated)
    );

    let fresh = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    let fresh_hash = hash_of(&fresh.handle);
    application
        .logout(
            &mut identities,
            &fresh_hash,
            request.now,
            &request.occurred_at,
        )
        .await?;
    for _ in 0..2 {
        assert_eq!(
            application.current(&fresh_hash, request.now).await,
            Err(ApplicationError::Unauthenticated)
        );
    }
    // Solo el digest se persiste; el handle en claro nunca llega a SQLite.
    let stored: Vec<String> = sqlx::query_scalar("SELECT session_hash FROM local_session")
        .fetch_all(&fixture.pool)
        .await?;
    assert!(stored.iter().all(|value| value.len() == 64));
    assert!(!stored.iter().any(|value| value == &fresh.handle));
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn branch_selection_accepts_only_active_branches_of_the_tenant()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    let application = identity_application(fixture.pool.clone())?;
    let mut identities = Identities;
    let request = request(3600);
    let session = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    let hash = hash_of(&session.handle);

    let selected = application
        .select_branch(
            &mut identities,
            &hash,
            request.now,
            &request.occurred_at,
            &id(BRANCH),
        )
        .await?;
    assert_eq!(selected.branch_id, Some(id(BRANCH)));

    assert_eq!(
        application
            .select_branch(
                &mut identities,
                &hash,
                request.now,
                &request.occurred_at,
                &id(INACTIVE_BRANCH)
            )
            .await,
        Err(ApplicationError::Forbidden)
    );
    assert_eq!(
        application
            .select_branch(
                &mut identities,
                &hash,
                request.now,
                &request.occurred_at,
                &id(FOREIGN_BRANCH)
            )
            .await,
        Err(ApplicationError::BranchNotFound)
    );
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn logout_all_invalidates_every_session_of_the_user() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = provisioned().await?;
    let application = identity_application(fixture.pool.clone())?;
    let mut identities = Identities;
    let request = request(3600);
    let first = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    let second = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    assert_ne!(first.handle, second.handle);

    let revoked = application
        .logout_all(
            &mut identities,
            &hash_of(&first.handle),
            request.now,
            &request.occurred_at,
        )
        .await?;
    assert_eq!(revoked, 2);
    for handle in [&first.handle, &second.handle] {
        assert_eq!(
            application.current(&hash_of(handle), request.now).await,
            Err(ApplicationError::Unauthenticated)
        );
    }
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn permission_removal_is_visible_on_the_next_operation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    let application = identity_application(fixture.pool.clone())?;
    let mut identities = Identities;
    let request = request(3600);
    let session = application
        .login(&mut identities, &request, PASSWORD)
        .await?;
    assert!(session.identity.permissions.contains("branch.read"));

    sqlx::query("DELETE FROM role_permission WHERE permission_code = 'branch.read'")
        .execute(&fixture.pool)
        .await?;
    let current = application
        .current(&hash_of(&session.handle), request.now)
        .await?;
    assert!(!current.permissions.contains("branch.read"));
    assert_eq!(
        application
            .select_branch(
                &mut identities,
                &hash_of(&session.handle),
                request.now,
                &request.occurred_at,
                &id(BRANCH)
            )
            .await,
        Err(ApplicationError::Forbidden)
    );
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn audit_failure_rolls_back_the_whole_login() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    sqlx::query(
        "CREATE TRIGGER reject_login_audit BEFORE INSERT ON audit_event \
         WHEN NEW.action = 'auth.login' \
         BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
    )
    .execute(&fixture.pool)
    .await?;
    assert_eq!(
        login(&fixture, PASSWORD).await.err(),
        Some(ApplicationError::Audit)
    );
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_session")
        .fetch_one(&fixture.pool)
        .await?;
    assert_eq!(sessions, 0);
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn concurrent_failed_attempts_do_not_corrupt_the_counter()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    let application = identity_application(fixture.pool.clone())?;
    let request = request(3600);
    let first = async {
        let mut identities = Identities;
        application
            .login(&mut identities, &request, "incorrecta")
            .await
    };
    let second = async {
        let mut identities = Identities;
        application
            .login(&mut identities, &request, "incorrecta")
            .await
    };
    let (a, b) = tokio::join!(first, second);
    assert!(a.is_err() && b.is_err());
    let attempts: i64 =
        sqlx::query_scalar("SELECT failed_login_attempts FROM local_credential WHERE user_id = ?")
            .bind(ADMIN)
            .fetch_one(&fixture.pool)
            .await?;
    assert!((1..=2).contains(&attempts), "intentos: {attempts}");
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_session")
        .fetch_one(&fixture.pool)
        .await?;
    assert_eq!(sessions, 0);
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn session_ttl_outside_policy_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = provisioned().await?;
    let application = identity_application(fixture.pool.clone())?;
    let mut identities = Identities;
    for ttl in [0, 59, MAX_SESSION_TTL_SECONDS + 1] {
        assert_eq!(
            application
                .login(&mut identities, &request(ttl), PASSWORD)
                .await
                .err(),
            Some(ApplicationError::InvalidSessionInput)
        );
    }
    fixture.pool.close().await;
    Ok(())
}
