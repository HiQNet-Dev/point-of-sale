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
use crate::identity_lab::{SqliteIdentityLab, ensure_lab_identity_schema};

const HASH: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture time: {error}"))
}
fn hash() -> SessionHash {
    SessionHash::new(HASH).unwrap_or_else(|error| panic!("fixture hash: {error}"))
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
const USER_B: &str = "00000000-0000-0000-0000-0000000000a1";
const ROLE_ORG: &str = "00000000-0000-0000-0000-0000000000b0";
const ROLE_BRANCH: &str = "00000000-0000-0000-0000-0000000000b1";
const MEMBERSHIP: &str = "00000000-0000-0000-0000-0000000000c0";
const SESSION: &str = "00000000-0000-0000-0000-0000000000d0";
const TIME: &str = "2026-10-29T00:00:00Z";

async fn seed(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    ensure_lab_identity_schema(pool).await?;
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    let org_a = organization(ORG_A, "lab-org-a");
    let org_b = organization(ORG_B, "lab-org-b");
    store.insert_organization(&org_a, TIME, TIME).await?;
    store.insert_organization(&org_b, TIME, TIME).await?;
    let branch_a1 = branch(BRANCH_A1, org_a.id().clone(), "lab-a1");
    let branch_a2 = branch(BRANCH_A2, org_a.id().clone(), "lab-a2");
    let branch_b1 = branch(BRANCH_B1, org_b.id().clone(), "lab-b1");
    store.insert_branch(&branch_a1, TIME, TIME).await?;
    store.insert_branch(&branch_a2, TIME, TIME).await?;
    store.insert_branch(&branch_b1, TIME, TIME).await?;
    for (user, owner, email) in [
        (USER_A, ORG_A, "operadora@lab.test"),
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
    ] {
        sqlx::query("INSERT INTO identity_role_lab (id, organization_id, code) VALUES (?, ?, ?)")
            .bind(role)
            .bind(owner)
            .bind(code)
            .execute(pool)
            .await?;
    }
    sqlx::query(
        "INSERT INTO identity_role_permission_lab (role_id, permission_code) VALUES (?, 'organization.read')",
    )
    .bind(ROLE_ORG)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO identity_role_permission_lab (role_id, permission_code) VALUES (?, 'branch.update')",
    )
    .bind(ROLE_BRANCH)
    .execute(pool)
    .await?;
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
    sqlx::query(
        "INSERT INTO identity_session_lab (id, user_id, session_hash, created_at, expires_at, revoked_at, session_version) \
         VALUES (?, ?, ?, 10, 1000, NULL, 1)",
    )
    .bind(SESSION)
    .bind(USER_A)
    .bind(HASH)
    .execute(pool)
    .await?;
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
    branch_id: Option<&str>,
    now: i64,
) -> Result<VerifiedPrincipal, AuditAttributionError> {
    let mut connection = pool
        .acquire()
        .await
        .unwrap_or_else(|error| panic!("pool: {error}"));
    let branch = branch_id.map(id);
    LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
        .authenticate(&hash(), moment(now), branch.as_ref())
        .await
}

#[tokio::test]
async fn lab_adapter_builds_scoped_principals_and_denies_other_scopes()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("identity-lab.sqlite")).await?;
    let mut connection = pool.acquire().await?;

    let principal = LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
        .authenticate(&hash(), moment(11), Some(&id(BRANCH_A1)))
        .await?;
    let scope_a1 = PrincipalScope::new(id(ORG_A), Some(id(BRANCH_A1)));
    assert_eq!(principal.scope().organization_id(), &id(ORG_A));
    assert_eq!(principal.scope().branch_id(), Some(&id(BRANCH_A1)));
    assert_eq!(
        principal.authorize(&permission("branch.update"), &scope_a1),
        Ok(())
    );
    assert_eq!(
        principal.authorize(&permission("organization.read"), &scope_a1),
        Ok(())
    );
    assert_eq!(
        principal.authorize(
            &permission("branch.update"),
            &PrincipalScope::new(id(ORG_A), Some(id(BRANCH_A2)))
        ),
        Err(AuthorizationError::ScopeMismatch)
    );
    assert_eq!(
        principal.authorize(
            &permission("branch.update"),
            &PrincipalScope::new(id(ORG_A), None)
        ),
        Err(AuthorizationError::ScopeMismatch)
    );

    let org_principal = LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
        .authenticate(&hash(), moment(11), None)
        .await?;
    assert_eq!(
        org_principal.authorize(
            &permission("organization.read"),
            &PrincipalScope::new(id(ORG_A), None)
        ),
        Ok(())
    );
    assert_eq!(
        org_principal.authorize(&permission("branch.update"), &scope_a1),
        Err(AuthorizationError::ScopeMismatch)
    );

    for (branch_id, expected) in [
        (BRANCH_A2, AuditAttributionError::ScopeDenied),
        (BRANCH_B1, AuditAttributionError::ScopeDenied),
    ] {
        assert_eq!(
            LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
                .authenticate(&hash(), moment(11), Some(&id(branch_id)))
                .await,
            Err(expected)
        );
    }
    Ok(())
}

#[tokio::test]
async fn lab_adapter_fails_closed_for_inactive_state_and_bad_sessions()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("identity-closed.sqlite")).await?;

    sqlx::query("UPDATE identity_membership_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&pool)
        .await?;
    assert_eq!(
        authenticate(&pool, Some(BRANCH_A1), 11).await,
        Err(AuditAttributionError::ScopeDenied)
    );

    sqlx::query("UPDATE identity_membership_lab SET status = 'ACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE branch SET status = 'INACTIVE' WHERE id = ?")
        .bind(BRANCH_A1)
        .execute(&pool)
        .await?;
    assert_eq!(
        authenticate(&pool, Some(BRANCH_A1), 11).await,
        Err(AuditAttributionError::ScopeDenied)
    );

    sqlx::query(
        "UPDATE identity_session_lab SET revoked_at = 20, session_version = 2 WHERE id = ?",
    )
    .bind(SESSION)
    .execute(&pool)
    .await?;
    assert_eq!(
        authenticate(&pool, None, 21).await,
        Err(AuditAttributionError::Unauthenticated)
    );

    sqlx::query(
        "UPDATE identity_session_lab SET revoked_at = NULL, session_version = 1 WHERE id = ?",
    )
    .bind(SESSION)
    .execute(&pool)
    .await?;
    assert_eq!(
        authenticate(&pool, None, 1_001).await,
        Err(AuditAttributionError::Unauthenticated)
    );

    sqlx::query("UPDATE identity_user_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(USER_A)
        .execute(&pool)
        .await?;
    assert_eq!(
        authenticate(&pool, None, 11).await,
        Err(AuditAttributionError::Unauthenticated)
    );
    Ok(())
}

#[tokio::test]
async fn lab_session_revocation_is_cas_and_single_writer_per_version()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("identity-cas.sqlite");
    let first = open_lab(&path).await?;
    let second = open(&path, Duration::from_millis(75)).await?;
    let mut first_connection = first.acquire().await?;
    let mut second_connection = second.acquire().await?;
    let session_id = id(SESSION);

    let (updated, conflicted) = tokio::join!(
        async {
            SqliteIdentityLab::new(&mut first_connection)
                .revoke_session_cas(&session_id, 1, 30)
                .await
        },
        async {
            SqliteIdentityLab::new(&mut second_connection)
                .revoke_session_cas(&session_id, 1, 30)
                .await
        }
    );
    let outcomes = [updated?, conflicted?];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == CasOutcome::Updated)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == CasOutcome::Conflict)
            .count(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT session_version FROM identity_session_lab WHERE id = ?"
        )
        .bind(SESSION)
        .fetch_one(&mut *first_connection)
        .await?,
        2
    );
    assert_eq!(
        SqliteIdentityLab::new(&mut first_connection)
            .revoke_session_cas(&id("00000000-0000-0000-0000-0000000000d9"), 1, 30)
            .await?,
        CasOutcome::Missing
    );
    drop(first_connection);
    drop(second_connection);
    first.close().await;
    second.close().await;
    Ok(())
}

#[tokio::test]
async fn lab_authorization_re_resolves_inside_the_unit_of_work_and_rolls_back()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_lab(&temp.path().join("identity-uow.sqlite")).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let mut transaction = unit_of_work.begin().await?;
    sqlx::query("UPDATE identity_membership_lab SET status = 'INACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&mut *transaction.connection())
        .await?;
    let denied = LocalPrincipalService::new(SqliteIdentityLab::new(transaction.connection()))
        .authenticate(&hash(), moment(11), Some(&id(BRANCH_A1)))
        .await;
    assert_eq!(denied, Err(AuditAttributionError::ScopeDenied));
    transaction.rollback().await?;

    let mut connection = pool.acquire().await?;
    let restored = LocalPrincipalService::new(SqliteIdentityLab::new(&mut connection))
        .authenticate(&hash(), moment(11), Some(&id(BRANCH_A1)))
        .await?;
    assert_eq!(restored.scope().branch_id(), Some(&id(BRANCH_A1)));
    drop(connection);

    let mut transaction = unit_of_work.begin().await?;
    let revoked = SqliteIdentityLab::new(transaction.connection())
        .revoke_session_cas(&id(SESSION), 1, 30)
        .await?;
    assert_eq!(revoked, CasOutcome::Updated);
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
    let cross_session = sqlx::query(
        "INSERT INTO identity_session_lab (id, user_id, session_hash, created_at, expires_at, revoked_at, session_version) \
         VALUES ('00000000-0000-0000-0000-0000000000d9', ?, 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 10, 100, NULL, 1)",
    )
    .bind(USER_B)
    .execute(&pool)
    .await;
    assert!(cross_session.is_ok());
    let mut connection = pool.acquire().await?;
    let derived = SqliteIdentityLab::new(&mut connection)
        .session_by_hash(
            &SessionHash::new("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                .unwrap_or_else(|error| panic!("hash: {error}")),
        )
        .await?;
    assert_eq!(
        derived.as_ref().map(|session| session.organization_id()),
        Some(&id(ORG_B))
    );
    Ok(())
}
