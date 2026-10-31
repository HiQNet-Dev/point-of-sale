// Fixtures de laboratorio; los panics solo señalan un error del propio test.
#![allow(clippy::panic)]

use std::{path::Path, time::Duration};

use pos_core::{
    audit::AuditAttributionError,
    identity::{
        AuthorizationError, LocalPrincipalService, PermissionCode, PrincipalScope, SessionHash,
        UtcSeconds, VerifiedPrincipal,
    },
    organization_branch::{
        Branch, BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewBranch,
        NewOrganization, Organization, Timezone,
    },
};

use super::*;
use crate::identity::{SqliteIdentityLab, ensure_lab_identity_schema};

const HASH: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

fn hash_a2() -> String {
    "b".repeat(64)
}
fn hash_b() -> String {
    "c".repeat(64)
}

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture time: {error}"))
}
fn hash(value: &str) -> SessionHash {
    SessionHash::new(value).unwrap_or_else(|error| panic!("fixture hash: {error}"))
}
fn organization(id_value: &str, code: &str) -> Organization {
    Organization::create(
        id(id_value),
        NewOrganization {
            name: DisplayName::new("Organización Lab")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("timezone: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    )
}
fn branch(id_value: &str, organization_id: EntityId, code: &str) -> Branch {
    Branch::create(
        id(id_value),
        organization_id,
        NewBranch {
            name: DisplayName::new("Sucursal Lab").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone_override: None,
        },
    )
}
fn permission(value: &str) -> PermissionCode {
    PermissionCode::new(value).unwrap_or_else(|error| panic!("permission: {error}"))
}

const ORG_A: &str = "00000000-0000-0000-0000-000000000080";
const ORG_B: &str = "00000000-0000-0000-0000-000000000081";
const BRANCH_A1: &str = "00000000-0000-0000-0000-000000000090";
const BRANCH_A2: &str = "00000000-0000-0000-0000-000000000091";
const BRANCH_B1: &str = "00000000-0000-0000-0000-000000000092";
const USER_A: &str = "00000000-0000-0000-0000-0000000000a0";
const USER_A2: &str = "00000000-0000-0000-0000-0000000000a2";
const USER_B: &str = "00000000-0000-0000-0000-0000000000a1";
const ROLE_ORG: &str = "00000000-0000-0000-0000-0000000000b0";
const ROLE_BRANCH: &str = "00000000-0000-0000-0000-0000000000b1";
const ROLE_ORG_OPS: &str = "00000000-0000-0000-0000-0000000000b2";
const MEMBERSHIP: &str = "00000000-0000-0000-0000-0000000000c0";
const SESSION: &str = "00000000-0000-0000-0000-0000000000d0";
const SESSION_A2: &str = "00000000-0000-0000-0000-0000000000d1";
const SESSION_B: &str = "00000000-0000-0000-0000-0000000000d3";
const TIME: &str = "2026-10-29T00:00:00Z";

async fn seed(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    ensure_lab_identity_schema(pool).await?;
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    let org_a = organization(ORG_A, "lab-org-a");
    let org_b = organization(ORG_B, "lab-org-b");
    store.insert_organization(&org_a, TIME, TIME).await?;
    store.insert_organization(&org_b, TIME, TIME).await?;
    for (branch_id, owner, code) in [
        (BRANCH_A1, org_a.id().clone(), "lab-a1"),
        (BRANCH_A2, org_a.id().clone(), "lab-a2"),
        (BRANCH_B1, org_b.id().clone(), "lab-b1"),
    ] {
        store
            .insert_branch(&branch(branch_id, owner, code), TIME, TIME)
            .await?;
    }
    for (user, owner, email) in [
        (USER_A, ORG_A, "operadora@lab.test"),
        (USER_A2, ORG_A, "supervisora@lab.test"),
        (USER_B, ORG_B, "otra@lab.test"),
    ] {
        sqlx::query(
            "INSERT INTO identity_user_lab (id, organization_id, email, display_name, status, version) \
             VALUES (?, ?, ?, 'Operadora Lab', 'ACTIVE', 0)",
        )
        .bind(user)
        .bind(owner)
        .bind(email)
        .execute(pool)
        .await?;
    }
    for (role, owner, code) in [
        (ROLE_ORG, ORG_A, "member-org"),
        (ROLE_BRANCH, ORG_A, "member-branch"),
        (ROLE_ORG_OPS, ORG_A, "org-operations"),
    ] {
        sqlx::query(
            "INSERT INTO identity_role_lab (id, organization_id, code, status) \
             VALUES (?, ?, ?, 'ACTIVE')",
        )
        .bind(role)
        .bind(owner)
        .bind(code)
        .execute(pool)
        .await?;
    }
    for (role, code) in [
        (ROLE_ORG, "organization.read"),
        (ROLE_BRANCH, "branch.update"),
        (ROLE_ORG_OPS, "branch.update"),
    ] {
        sqlx::query(
            "INSERT INTO identity_role_permission_lab (role_id, permission_code) VALUES (?, ?)",
        )
        .bind(role)
        .bind(code)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO identity_user_role_lab (organization_id, user_id, role_id) VALUES (?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(USER_A)
    .bind(ROLE_ORG)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO identity_membership_lab (id, organization_id, branch_id, user_id, status) \
         VALUES (?, ?, ?, ?, 'ACTIVE')",
    )
    .bind(MEMBERSHIP)
    .bind(ORG_A)
    .bind(BRANCH_A1)
    .bind(USER_A)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO identity_membership_role_lab (organization_id, membership_id, role_id) \
         VALUES (?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(MEMBERSHIP)
    .bind(ROLE_BRANCH)
    .execute(pool)
    .await?;
    for (session, user, session_hash) in [
        (SESSION, USER_A, HASH.to_owned()),
        (SESSION_A2, USER_A2, hash_a2()),
        (SESSION_B, USER_B, hash_b()),
    ] {
        sqlx::query(
            "INSERT INTO identity_session_lab (id, user_id, session_hash, created_at, expires_at, revoked_at, session_version) \
             VALUES (?, ?, ?, 10, 1000, NULL, 1)",
        )
        .bind(session)
        .bind(user)
        .bind(session_hash)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn open_lab(path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = open(path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    seed(&pool).await?;
    Ok(pool)
}

async fn authenticate(
    pool: &SqlitePool,
    session_hash: &str,
    branch_id: Option<&str>,
    now: i64,
) -> Result<VerifiedPrincipal, AuditAttributionError> {
    let mut connection = pool
        .acquire()
        .await
        .unwrap_or_else(|error| panic!("pool: {error}"));
    let branch = branch_id.map(id);
    LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
        .authenticate(&hash(session_hash), moment(now), branch.as_ref())
        .await
}

async fn authenticate_recovery(
    pool: &SqlitePool,
    session_hash: &str,
    branch_id: &str,
    now: i64,
) -> Result<VerifiedPrincipal, AuditAttributionError> {
    let mut connection = pool
        .acquire()
        .await
        .unwrap_or_else(|error| panic!("pool: {error}"));
    LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
        .authenticate_recovery(&hash(session_hash), moment(now), &id(branch_id))
        .await
}

fn scope(branch_id: Option<&str>) -> PrincipalScope {
    PrincipalScope::new(id(ORG_A), branch_id.map(id))
}

async fn grant_organization_branch_update(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO identity_user_role_lab (organization_id, user_id, role_id) VALUES (?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(USER_A)
    .bind(ROLE_ORG_OPS)
    .execute(pool)
    .await
    .unwrap_or_else(|error| panic!("org grant: {error}"));
}

#[tokio::test]
async fn java_parity_organization_override_grants_branch_without_membership()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-override.sqlite")).await?;
    grant_organization_branch_update(&pool).await;

    // Sin membership en A2: Java concede por override de organización.
    let principal = authenticate(&pool, HASH, Some(BRANCH_A2), 11).await?;
    assert_eq!(
        principal.authorize(&permission("branch.update"), &scope(Some(BRANCH_A2))),
        Ok(())
    );
    // También sobre una sucursal inexistente: la autorización no revela existencia.
    let unknown = "00000000-0000-0000-0000-0000000000ff";
    let principal = authenticate(&pool, HASH, Some(unknown), 11).await?;
    assert_eq!(
        principal.authorize(&permission("branch.update"), &scope(Some(unknown))),
        Ok(())
    );
    Ok(())
}

#[tokio::test]
async fn java_parity_branch_grant_does_not_promote_to_organization_scope()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-promotion.sqlite")).await?;

    let org_principal = authenticate(&pool, HASH, None, 11).await?;
    assert_eq!(
        org_principal.authorize(&permission("organization.read"), &scope(None)),
        Ok(())
    );
    assert_eq!(
        org_principal.authorize(&permission("branch.update"), &scope(None)),
        Err(AuthorizationError::PermissionDenied)
    );

    let branch_principal = authenticate(&pool, HASH, Some(BRANCH_A1), 11).await?;
    assert_eq!(
        branch_principal.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Ok(())
    );
    assert_eq!(
        branch_principal.authorize(
            &permission("branch.update"),
            &PrincipalScope::new(id(ORG_A), Some(id(BRANCH_A2)))
        ),
        Err(AuthorizationError::ScopeMismatch)
    );
    Ok(())
}

#[tokio::test]
async fn java_parity_unknown_or_foreign_branch_grants_nothing_without_error()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-foreign.sqlite")).await?;

    let foreign = authenticate(&pool, HASH, Some(BRANCH_B1), 11).await?;
    assert_eq!(foreign.scope().organization_id(), &id(ORG_A));
    assert_eq!(
        foreign.authorize(&permission("branch.update"), &scope(Some(BRANCH_B1))),
        Err(AuthorizationError::PermissionDenied)
    );
    let no_membership = authenticate(&pool, HASH, Some(BRANCH_A2), 11).await?;
    assert_eq!(
        no_membership.authorize(&permission("branch.update"), &scope(Some(BRANCH_A2))),
        Err(AuthorizationError::PermissionDenied)
    );
    Ok(())
}

#[tokio::test]
async fn java_parity_inactive_role_organization_and_membership_revoke_grants()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-inactive.sqlite")).await?;

    sqlx::query("UPDATE identity_role_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(ROLE_ORG)
        .execute(&pool)
        .await?;
    let no_role = authenticate(&pool, HASH, None, 11).await?;
    assert_eq!(
        no_role.authorize(&permission("organization.read"), &scope(None)),
        Err(AuthorizationError::PermissionDenied)
    );
    sqlx::query("UPDATE identity_role_lab SET status = 'ACTIVE' WHERE id = ?")
        .bind(ROLE_ORG)
        .execute(&pool)
        .await?;

    sqlx::query("UPDATE identity_membership_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&pool)
        .await?;
    let no_membership = authenticate(&pool, HASH, Some(BRANCH_A1), 11).await?;
    assert_eq!(
        no_membership.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Err(AuthorizationError::PermissionDenied)
    );
    assert_eq!(
        no_membership.authorize(&permission("organization.read"), &scope(Some(BRANCH_A1))),
        Ok(())
    );
    sqlx::query("UPDATE identity_membership_lab SET status = 'ACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&pool)
        .await?;

    sqlx::query("UPDATE organization SET status = 'INACTIVE' WHERE id = ?")
        .bind(ORG_A)
        .execute(&pool)
        .await?;
    let inactive_org = authenticate(&pool, HASH, None, 11).await?;
    assert_eq!(
        inactive_org.authorize(&permission("organization.read"), &scope(None)),
        Err(AuthorizationError::PermissionDenied)
    );
    let inactive_org_branch = authenticate(&pool, HASH, Some(BRANCH_A1), 11).await?;
    assert_eq!(
        inactive_org_branch.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Err(AuthorizationError::PermissionDenied)
    );
    Ok(())
}

#[tokio::test]
async fn java_parity_recovery_authorizes_inactive_branch_but_normal_does_not()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-recovery.sqlite")).await?;

    sqlx::query("UPDATE branch SET status = 'INACTIVE' WHERE id = ?")
        .bind(BRANCH_A1)
        .execute(&pool)
        .await?;
    let normal = authenticate(&pool, HASH, Some(BRANCH_A1), 11).await?;
    assert_eq!(
        normal.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Err(AuthorizationError::PermissionDenied)
    );

    let recovery = authenticate_recovery(&pool, HASH, BRANCH_A1, 11).await?;
    let recovery_scope = PrincipalScope::recovery(id(ORG_A), id(BRANCH_A1));
    assert_eq!(
        recovery.authorize(&permission("branch.update"), &recovery_scope),
        Ok(())
    );
    // El principal de recuperación no sirve para operaciones normales.
    assert_eq!(
        recovery.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Err(AuthorizationError::ScopeMismatch)
    );
    Ok(())
}

#[tokio::test]
async fn java_parity_inactive_or_foreign_states_fail_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-session.sqlite")).await?;

    sqlx::query("UPDATE identity_user_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(USER_A2)
        .execute(&pool)
        .await?;
    assert_eq!(
        authenticate(&pool, &hash_a2(), None, 11).await,
        Err(AuditAttributionError::Unauthenticated)
    );

    sqlx::query(
        "UPDATE identity_session_lab SET revoked_at = 20, session_version = 2 WHERE id = ?",
    )
    .bind(SESSION)
    .execute(&pool)
    .await?;
    assert_eq!(
        authenticate(&pool, HASH, None, 21).await,
        Err(AuditAttributionError::Unauthenticated)
    );

    sqlx::query(
        "UPDATE identity_session_lab SET revoked_at = NULL, session_version = 1 WHERE id = ?",
    )
    .bind(SESSION)
    .execute(&pool)
    .await?;
    assert_eq!(
        authenticate(&pool, HASH, None, 1_001).await,
        Err(AuditAttributionError::Unauthenticated)
    );
    Ok(())
}

#[tokio::test]
async fn java_parity_concurrent_permission_changes_are_seen_on_reauthentication()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-concurrent.sqlite")).await?;

    let before = authenticate(&pool, HASH, Some(BRANCH_A1), 11).await?;
    assert_eq!(
        before.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Ok(())
    );
    sqlx::query("UPDATE identity_role_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(ROLE_BRANCH)
        .execute(&pool)
        .await?;
    let after = authenticate(&pool, HASH, Some(BRANCH_A1), 12).await?;
    assert_eq!(
        after.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Err(AuthorizationError::PermissionDenied)
    );
    // El principal anterior no se reutiliza: una decisión ya tomada no autoriza
    // una mutación posterior; el caso de uso debe re-autenticar en la UoW.
    assert_eq!(
        before.authorize(&permission("branch.update"), &scope(Some(BRANCH_A1))),
        Ok(())
    );
    Ok(())
}

#[tokio::test]
async fn java_parity_storage_failure_during_authorization_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("parity-storage.sqlite")).await?;

    sqlx::query("DROP TABLE identity_role_permission_lab")
        .execute(&pool)
        .await?;
    assert_eq!(
        authenticate(&pool, HASH, Some(BRANCH_A1), 11).await,
        Err(AuditAttributionError::ScopeDenied)
    );
    assert_eq!(
        authenticate_recovery(&pool, HASH, BRANCH_A1, 11).await,
        Err(AuditAttributionError::ScopeDenied)
    );
    Ok(())
}

#[tokio::test]
async fn session_revocation_is_scoped_to_the_authenticated_principal()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("revocation-scope.sqlite")).await?;
    let principal = authenticate(&pool, HASH, None, 11).await?;
    let mut connection = pool.acquire().await?;

    assert_eq!(
        SqliteIdentityLab::new(&mut connection)
            .revoke_session_cas(&principal, &id(SESSION), 1, 30)
            .await?,
        CasOutcome::Updated
    );
    assert_eq!(
        SqliteIdentityLab::new(&mut connection)
            .revoke_session_cas(&principal, &id(SESSION), 1, 31)
            .await?,
        CasOutcome::Conflict
    );
    // Sesión de otro usuario de la misma organización: no existe para el principal.
    assert_eq!(
        SqliteIdentityLab::new(&mut connection)
            .revoke_session_cas(&principal, &id(SESSION_A2), 1, 30)
            .await?,
        CasOutcome::Missing
    );
    // Sesión de otra organización y sesión inexistente responden igual.
    assert_eq!(
        SqliteIdentityLab::new(&mut connection)
            .revoke_session_cas(&principal, &id(SESSION_B), 1, 30)
            .await?,
        CasOutcome::Missing
    );
    assert_eq!(
        SqliteIdentityLab::new(&mut connection)
            .revoke_session_cas(
                &principal,
                &id("00000000-0000-0000-0000-0000000000d9"),
                1,
                30
            )
            .await?,
        CasOutcome::Missing
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT session_version FROM identity_session_lab WHERE id = ?"
        )
        .bind(SESSION)
        .fetch_one(&mut *connection)
        .await?,
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT revoked_at FROM identity_session_lab WHERE id = ?"
        )
        .bind(SESSION_B)
        .fetch_one(&mut *connection)
        .await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn session_revocation_and_authorization_share_the_unit_of_work()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("revocation-uow.sqlite")).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut transaction = unit_of_work.begin().await?;
    sqlx::query("UPDATE identity_membership_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&mut *transaction.connection())
        .await?;
    let denied = LocalPrincipalService::new(SqliteIdentityLab::new(transaction.connection()))
        .authenticate(&hash(HASH), moment(11), Some(&id(BRANCH_A1)))
        .await?
        .authorize(&permission("branch.update"), &scope(Some(BRANCH_A1)));
    assert_eq!(denied, Err(AuthorizationError::PermissionDenied));
    transaction.rollback().await?;

    let principal = authenticate(&pool, HASH, None, 11).await?;
    let mut transaction = unit_of_work.begin().await?;
    assert_eq!(
        SqliteIdentityLab::new(transaction.connection())
            .revoke_session_cas(&principal, &id(SESSION), 1, 30)
            .await?,
        CasOutcome::Updated
    );
    transaction.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT revoked_at FROM identity_session_lab WHERE id = ?"
        )
        .bind(SESSION)
        .fetch_one(&pool)
        .await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn lab_schema_enforces_cross_organization_ownership() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("identity-fk.sqlite")).await?;
    let cross_membership = sqlx::query(
        "INSERT INTO identity_membership_lab (id, organization_id, branch_id, user_id, status) \
         VALUES ('00000000-0000-0000-0000-0000000000c9', ?, ?, ?, 'ACTIVE')",
    )
    .bind(ORG_A)
    .bind(BRANCH_B1)
    .bind(USER_A)
    .execute(&pool)
    .await;
    assert!(cross_membership.is_err());
    let cross_role = sqlx::query(
        "INSERT INTO identity_user_role_lab (organization_id, user_id, role_id) VALUES (?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(USER_B)
    .bind(ROLE_ORG)
    .execute(&pool)
    .await;
    assert!(cross_role.is_err());
    let mut connection = pool.acquire().await?;
    let derived = SqliteIdentityLab::new(&mut connection)
        .session_by_hash(&hash(&hash_b()))
        .await?;
    assert_eq!(
        derived.as_ref().map(|session| session.organization_id()),
        Some(&id(ORG_B))
    );
    Ok(())
}
