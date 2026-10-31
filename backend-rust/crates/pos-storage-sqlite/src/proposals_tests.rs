// Valida las propuestas físicas fuera de producto: sintaxis, CHECK, FKs,
// triggers, semántica RBAC y durabilidad. No las convierte en migraciones.
#![allow(clippy::panic)]

use std::{collections::BTreeSet, time::Duration};

use pos_core::{
    identity::PERMISSION_CATALOG,
    organization_branch::{
        Branch, BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewBranch,
        NewOrganization, Organization, Timezone,
    },
};

use super::*;

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
const PROPOSAL_AUTHORIZATION: &str =
    include_str!("../../../proposals/local-auth/0017_bootstrap_authorization.proposed.sql");

const ORG: &str = "00000000-0000-0000-0000-000000000080";
const ORG_TWO: &str = "00000000-0000-0000-0000-000000000081";
const BRANCH_A: &str = "00000000-0000-0000-0000-000000000090";
const USER: &str = "00000000-0000-0000-0000-0000000000a0";
const USER_TWO: &str = "00000000-0000-0000-0000-0000000000a1";
const CREDENTIAL: &str = "00000000-0000-0000-0000-0000000000e0";
const SESSION: &str = "00000000-0000-0000-0000-0000000000d0";
const ROLE_ORG: &str = "00000000-0000-0000-0000-0000000000b0";
const ROLE_BRANCH: &str = "00000000-0000-0000-0000-0000000000b1";
const MEMBERSHIP: &str = "00000000-0000-0000-0000-0000000000c0";
const INSTALLATION: &str = "00000000-0000-0000-0000-0000000000f0";
const INSTALLATION_TWO: &str = "00000000-0000-0000-0000-0000000000f1";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const HASH: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn organization(value: &str, code: &str) -> Organization {
    Organization::create(
        id(value),
        NewOrganization {
            name: DisplayName::new("Propuesta").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    )
}

async fn apply_proposals(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for statement in [
        PROPOSAL_INSTALLATION,
        PROPOSAL_IDENTITY,
        PROPOSAL_RBAC,
        PROPOSAL_MEMBERSHIP,
        PROPOSAL_AUDIT,
        PROPOSAL_BOOTSTRAP,
        PROPOSAL_AUTHORIZATION,
    ] {
        sqlx::raw_sql(statement).execute(pool).await?;
    }
    Ok(())
}

async fn open_proposed(path: &std::path::Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = open(path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    apply_proposals(&pool).await?;
    Ok(pool)
}

async fn seed(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    for (value, code) in [(ORG, "prop-org-a"), (ORG_TWO, "prop-org-b")] {
        store
            .insert_organization(&organization(value, code), TIME, TIME)
            .await?;
    }
    for (installation, owner, fingerprint) in [
        (INSTALLATION, ORG, "ed25519:test-fingerprint-a"),
        (INSTALLATION_TWO, ORG, "ed25519:test-fingerprint-b"),
    ] {
        sqlx::query(
            "INSERT INTO installation_identity (id, organization_id, public_key_fingerprint, initialized_at, updated_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(installation)
        .bind(owner)
        .bind(fingerprint)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    let branch = Branch::create(
        id(BRANCH_A),
        id(ORG),
        NewBranch {
            name: DisplayName::new("Sucursal").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("prop-branch").unwrap_or_else(|error| panic!("code: {error}")),
            timezone_override: None,
        },
    );
    store.insert_branch(&branch, TIME, TIME).await?;
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
         VALUES (?, ?, ?, 'PASSWORD', 'LEGACY_BCRYPT', '$2b$04$synthetic', '{\"cost\":4}', 'legacy-import-v1', 'ACTIVE', 1, 0, ?, ?)",
    )
    .bind(CREDENTIAL)
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

async fn complete_bootstrap(
    pool: &SqlitePool,
    installation: &str,
) -> Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "UPDATE bootstrap_record SET state = 'COMPLETED', completed_at = ?, organization_id = ?, \
         administrator_user_id = ?, administrator_role_id = ?, bootstrap_version = 1, updated_at = ? \
         WHERE installation_id = ? AND state = 'PENDING'",
    )
    .bind(LATER)
    .bind(ORG)
    .bind(USER)
    .bind(ROLE_ORG)
    .bind(LATER)
    .bind(installation)
    .execute(pool)
    .await
}

async fn effective_permissions(
    pool: &SqlitePool,
    user: &str,
    organization: &str,
    branch: Option<&str>,
    require_active_branch: bool,
) -> Result<Vec<String>, sqlx::Error> {
    let mut granted: BTreeSet<String> =
        sqlx::query_scalar::<_, String>(ORGANIZATION_PERMISSIONS_SQL)
            .bind(organization)
            .bind(user)
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    if let Some(branch) = branch {
        let branch_grants: Vec<String> = if require_active_branch {
            sqlx::query_scalar::<_, String>(BRANCH_PERMISSIONS_SQL)
                .bind(organization)
                .bind(branch)
                .bind(user)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_scalar::<_, String>(BRANCH_PERMISSIONS_RECOVERY_SQL)
                .bind(organization)
                .bind(branch)
                .bind(user)
                .fetch_all(pool)
                .await?
        };
        granted.extend(branch_grants);
    }
    Ok(granted.into_iter().collect())
}

const ORGANIZATION_PERMISSIONS_SQL: &str = "SELECT DISTINCT rp.permission_code FROM user_organization_role ur \
     JOIN pos_user u ON u.id = ur.user_id AND u.organization_id = ur.organization_id \
     JOIN role r ON r.id = ur.role_id AND r.organization_id = ur.organization_id \
     JOIN role_permission rp ON rp.role_id = r.id \
     JOIN organization o ON o.id = ur.organization_id \
     WHERE ur.organization_id = ? AND ur.user_id = ? \
       AND u.status = 'ACTIVE' AND r.status = 'ACTIVE' AND o.status = 'ACTIVE' \
     ORDER BY rp.permission_code";

const BRANCH_PERMISSIONS_SQL: &str = "SELECT DISTINCT rp.permission_code FROM branch_membership m \
     JOIN pos_user u ON u.id = m.user_id AND u.organization_id = m.organization_id \
     JOIN branch b ON b.id = m.branch_id AND b.organization_id = m.organization_id \
     JOIN branch_membership_role mr ON mr.membership_id = m.id AND mr.organization_id = m.organization_id \
     JOIN role r ON r.id = mr.role_id AND r.organization_id = mr.organization_id \
     JOIN role_permission rp ON rp.role_id = r.id \
     JOIN organization o ON o.id = m.organization_id \
     WHERE m.organization_id = ? AND m.branch_id = ? AND m.user_id = ? \
       AND m.status = 'ACTIVE' AND u.status = 'ACTIVE' AND r.status = 'ACTIVE' \
       AND o.status = 'ACTIVE' AND b.status = 'ACTIVE' \
     ORDER BY rp.permission_code";

const BRANCH_PERMISSIONS_RECOVERY_SQL: &str = "SELECT DISTINCT rp.permission_code FROM branch_membership m \
     JOIN pos_user u ON u.id = m.user_id AND u.organization_id = m.organization_id \
     JOIN branch b ON b.id = m.branch_id AND b.organization_id = m.organization_id \
     JOIN branch_membership_role mr ON mr.membership_id = m.id AND mr.organization_id = m.organization_id \
     JOIN role r ON r.id = mr.role_id AND r.organization_id = mr.organization_id \
     JOIN role_permission rp ON rp.role_id = r.id \
     JOIN organization o ON o.id = m.organization_id \
     WHERE m.organization_id = ? AND m.branch_id = ? AND m.user_id = ? \
       AND m.status = 'ACTIVE' AND u.status = 'ACTIVE' AND r.status = 'ACTIVE' \
       AND o.status = 'ACTIVE' \
     ORDER BY rp.permission_code";

async fn seed_rbac(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for (role, code) in [(ROLE_ORG, "org-role"), (ROLE_BRANCH, "branch-role")] {
        sqlx::query(
            "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, 'Rol', 'ACTIVE', 0, ?, ?)",
        )
        .bind(role)
        .bind(ORG)
        .bind(code)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    for (role, permission) in [
        (ROLE_ORG, "organization.read"),
        (ROLE_ORG, "branch.update"),
        (ROLE_BRANCH, "sale.create"),
    ] {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(role)
            .bind(permission)
            .execute(pool)
            .await?;
    }
    sqlx::query(
        "INSERT INTO user_organization_role (organization_id, user_id, role_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(ORG)
    .bind(USER)
    .bind(ROLE_ORG)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO branch_membership (id, organization_id, branch_id, user_id, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'ACTIVE', 0, ?, ?)",
    )
    .bind(MEMBERSHIP)
    .bind(ORG)
    .bind(BRANCH_A)
    .bind(USER)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO branch_membership_role (organization_id, membership_id, role_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(ORG)
    .bind(MEMBERSHIP)
    .bind(ROLE_BRANCH)
    .bind(TIME)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn proposal_set_applies_from_empty_database_and_rejects_reapplication()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("proposals.sqlite");
    let pool = open_proposed(&path).await?;
    seed(&pool).await?;
    // Reaplicar una migración ya ejecutada debe fallar: contrato one-shot.
    assert!(
        sqlx::raw_sql(PROPOSAL_IDENTITY)
            .execute(&pool)
            .await
            .is_err()
    );

    pool.close().await;
    let reopened = open(&path, Duration::from_millis(75)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pos_user")
            .fetch_one(&reopened)
            .await?,
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_session")
            .fetch_one(&reopened)
            .await?,
        1
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn catalog_matches_the_java_permission_list_exactly() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("catalog.sqlite")).await?;
    let mut stored = sqlx::query_scalar::<_, String>("SELECT code FROM permission ORDER BY code")
        .fetch_all(&pool)
        .await?;
    let mut expected: Vec<String> = PERMISSION_CATALOG
        .iter()
        .map(|code| (*code).to_owned())
        .collect();
    expected.sort();
    assert_eq!(stored, expected);
    assert_eq!(stored.len(), 59);

    // El seed es idempotente y no altera descripciones existentes.
    sqlx::raw_sql(
        "INSERT INTO permission (code, description) VALUES ('organization.read', 'duplicado') \
         ON CONFLICT (code) DO NOTHING",
    )
    .execute(&pool)
    .await?;
    stored = sqlx::query_scalar::<_, String>(
        "SELECT description FROM permission WHERE code = 'organization.read'",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(stored, vec!["Read organization details".to_owned()]);
    Ok(())
}

#[tokio::test]
async fn normalization_uniqueness_and_uuid_rules_hold() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("identity.sqlite")).await?;
    seed(&pool).await?;

    // Email único por organización.
    assert!(
        sqlx::query(
            "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000000a2', ?, 'Otra', 'operadora@prop.test', 'ACTIVE', 0, ?, ?)",
        )
        .bind(ORG)
        .bind(TIME)
        .bind(TIME)
        .execute(&pool)
        .await
        .is_err()
    );
    // Mismo email en otra organización es válido.
    sqlx::query(
        "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000000a3', ?, 'Otra', 'operadora@prop.test', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG_TWO)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await?;
    // UUID no canónico (mayúsculas) rechazado.
    assert!(
        sqlx::query(
            "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000000AF', ?, 'Otra', 'x@prop.test', 'ACTIVE', 0, ?, ?)",
        )
        .bind(ORG)
        .bind(TIME)
        .bind(TIME)
        .execute(&pool)
        .await
        .is_err()
    );
    // Timestamp sin 'Z' rechazado.
    assert!(
        sqlx::query(
            "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000000a4', ?, 'Otra', 'y@prop.test', 'ACTIVE', 0, '2026-10-29 00:00:00', ?)",
        )
        .bind(ORG)
        .bind(TIME)
        .execute(&pool)
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn session_state_is_coherent_and_expiry_is_derived() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("session.sqlite")).await?;
    seed(&pool).await?;

    // ACTIVE no puede llevar revoked_at.
    assert!(
        sqlx::query("UPDATE local_session SET revoked_at = ? WHERE id = ?")
            .bind(LATER)
            .bind(SESSION)
            .execute(&pool)
            .await
            .is_err()
    );
    // REVOKED exige revoked_at.
    assert!(
        sqlx::query("UPDATE local_session SET state = 'REVOKED' WHERE id = ?")
            .bind(SESSION)
            .execute(&pool)
            .await
            .is_err()
    );
    // EXPIRED no puede llevar revoked_at (revocación explícita gana).
    assert!(
        sqlx::query("UPDATE local_session SET state = 'EXPIRED', revoked_at = ? WHERE id = ?")
            .bind(LATER)
            .bind(SESSION)
            .execute(&pool)
            .await
            .is_err()
    );
    // Transición de revocación atómica con CAS.
    let first = sqlx::query(
        "UPDATE local_session SET state = 'REVOKED', revoked_at = ?, session_version = session_version + 1 \
         WHERE id = ? AND session_version = 1 AND state = 'ACTIVE'",
    )
    .bind(LATER)
    .bind(SESSION)
    .execute(&pool)
    .await?;
    assert_eq!(first.rows_affected(), 1);
    let second = sqlx::query(
        "UPDATE local_session SET state = 'REVOKED', revoked_at = ?, session_version = session_version + 1 \
         WHERE id = ? AND session_version = 1 AND state = 'ACTIVE'",
    )
    .bind(LATER)
    .bind(SESSION)
    .execute(&pool)
    .await?;
    assert_eq!(second.rows_affected(), 0);

    // Expiración derivada: una fila ACTIVE con expires_at vencido no autentica.
    sqlx::query(
        "INSERT INTO local_session (id, user_id, session_hash, state, created_at, expires_at, revoked_at, session_version) \
         VALUES ('00000000-0000-0000-0000-0000000000d1', ?, ?, 'ACTIVE', ?, ?, NULL, 1)",
    )
    .bind(USER)
    .bind("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    .bind(TIME)
    .bind("2026-10-29T12:00:00Z")
    .execute(&pool)
    .await?;
    let usable = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM local_session \
         WHERE id = '00000000-0000-0000-0000-0000000000d1' \
           AND state = 'ACTIVE' AND revoked_at IS NULL AND expires_at > ?",
    )
    .bind(LATER)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        usable, 0,
        "la expiración no depende de materializar EXPIRED"
    );
    Ok(())
}

#[tokio::test]
async fn bootstrap_is_one_shot_per_installation_and_complete()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("bootstrap.sqlite")).await?;
    seed(&pool).await?;
    seed_rbac(&pool).await?;

    sqlx::query(
        "INSERT INTO bootstrap_record (installation_id, state, updated_at) VALUES (?, 'PENDING', ?)",
    )
    .bind(INSTALLATION)
    .bind(TIME)
    .execute(&pool)
    .await?;
    // Provisioning parcial rechazado.
    assert!(
        sqlx::query("UPDATE bootstrap_record SET completed_at = ? WHERE installation_id = ?")
            .bind(LATER)
            .bind(INSTALLATION)
            .execute(&pool)
            .await
            .is_err()
    );
    complete_bootstrap(&pool, INSTALLATION).await?;
    // Segundo intento para la misma instalación no actualiza nada.
    assert_eq!(
        complete_bootstrap(&pool, INSTALLATION)
            .await?
            .rows_affected(),
        0
    );
    // Otra instalación tiene su propio registro one-shot.
    sqlx::query(
        "INSERT INTO bootstrap_record (installation_id, state, updated_at) VALUES (?, 'PENDING', ?)",
    )
    .bind(INSTALLATION_TWO)
    .bind(TIME)
    .execute(&pool)
    .await?;

    // Instalación de Organization A no puede completar con Organization B.
    sqlx::query(
        "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000000b2', ?, 'org-b-role', 'Rol B', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG_TWO)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await?;
    let cross = sqlx::query(
        "UPDATE bootstrap_record SET state = 'COMPLETED', completed_at = ?, organization_id = ?, \
         administrator_user_id = ?, administrator_role_id = ?, updated_at = ? \
         WHERE installation_id = ? AND state = 'PENDING'",
    )
    .bind(LATER)
    .bind(ORG_TWO)
    .bind(USER_TWO)
    .bind("00000000-0000-0000-0000-0000000000b2")
    .bind(LATER)
    .bind(INSTALLATION_TWO)
    .execute(&pool)
    .await;
    assert!(
        cross.is_err(),
        "la instalación y su organización deben pertenecer entre sí"
    );

    // Fallo de auditoría revierte el bootstrap dentro de la misma transacción.
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    sqlx::query(
        "UPDATE bootstrap_record SET state = 'COMPLETED', completed_at = ?, organization_id = ?, \
         administrator_user_id = ?, administrator_role_id = ?, updated_at = ? WHERE installation_id = ?",
    )
    .bind(LATER)
    .bind(ORG)
    .bind(USER)
    .bind(ROLE_ORG)
    .bind(LATER)
    .bind(INSTALLATION_TWO)
    .execute(&mut *transaction.connection())
    .await?;
    let audit = sqlx::query(
        "INSERT INTO audit_event (id, organization_id, actor_type, actor_user_id, session_id, action, entity_type, occurred_at, event_version) \
         VALUES ('00000000-0000-0000-0000-0000000000f9', ?, 'SYSTEM', ?, NULL, 'bootstrap.completed', 'bootstrap_record', ?, 1)",
    )
    .bind(ORG)
    .bind(USER)
    .bind(LATER)
    .execute(&mut *transaction.connection())
    .await;
    assert!(audit.is_err(), "SYSTEM con actor debe fallar");
    transaction.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM bootstrap_record WHERE installation_id = ?"
        )
        .bind(INSTALLATION_TWO)
        .fetch_one(&pool)
        .await?,
        "PENDING"
    );
    Ok(())
}

#[tokio::test]
async fn rbac_catalog_resolution_preserves_java_semantics() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("rbac.sqlite")).await?;
    seed(&pool).await?;
    seed_rbac(&pool).await?;

    let organization = effective_permissions(&pool, USER, ORG, None, true).await?;
    assert_eq!(organization, vec!["branch.update", "organization.read"]);

    let branch = effective_permissions(&pool, USER, ORG, Some(BRANCH_A), true).await?;
    assert_eq!(
        branch,
        vec!["branch.update", "organization.read", "sale.create"]
    );

    // Override de organización sin membership: la pierna de sucursal no aporta,
    // pero las grants organizacionales se conservan (Java).
    sqlx::query("UPDATE branch_membership SET status = 'INACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&pool)
        .await?;
    let without_membership = effective_permissions(&pool, USER, ORG, Some(BRANCH_A), true).await?;
    assert_eq!(
        without_membership,
        vec!["branch.update", "organization.read"]
    );

    // El permiso de sucursal nunca aparece en el scope de organización.
    assert!(!organization.contains(&"sale.create".to_owned()));

    // Rol inactivo retira solo su propia grant.
    sqlx::query("UPDATE branch_membership SET status = 'ACTIVE' WHERE id = ?")
        .bind(MEMBERSHIP)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE role SET status = 'INACTIVE' WHERE id = ?")
        .bind(ROLE_BRANCH)
        .execute(&pool)
        .await?;
    let inactive_role = effective_permissions(&pool, USER, ORG, Some(BRANCH_A), true).await?;
    assert_eq!(inactive_role, vec!["branch.update", "organization.read"]);
    sqlx::query("UPDATE role SET status = 'ACTIVE' WHERE id = ?")
        .bind(ROLE_BRANCH)
        .execute(&pool)
        .await?;

    // Sucursal inactiva: la variante normal no aporta la grant de sucursal; la
    // histórica sí.
    sqlx::query("UPDATE branch SET status = 'INACTIVE' WHERE id = ?")
        .bind(BRANCH_A)
        .execute(&pool)
        .await?;
    assert_eq!(
        effective_permissions(&pool, USER, ORG, Some(BRANCH_A), true).await?,
        vec!["branch.update", "organization.read"]
    );
    assert_eq!(
        effective_permissions(&pool, USER, ORG, Some(BRANCH_A), false).await?,
        vec!["branch.update", "organization.read", "sale.create"]
    );

    // Organización inactiva retira todo.
    sqlx::query("UPDATE branch SET status = 'ACTIVE' WHERE id = ?")
        .bind(BRANCH_A)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE organization SET status = 'INACTIVE' WHERE id = ?")
        .bind(ORG)
        .execute(&pool)
        .await?;
    assert_eq!(
        effective_permissions(&pool, USER, ORG, None, true).await?,
        Vec::<String>::new()
    );
    Ok(())
}

#[tokio::test]
async fn audit_contract_enforces_actor_session_and_append_only()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("audit.sqlite")).await?;
    seed(&pool).await?;

    let insert = |id_value: &'static str,
                  actor: &'static str,
                  user: Option<&'static str>,
                  session: Option<&'static str>| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO audit_event (id, organization_id, branch_id, actor_type, actor_user_id, session_id, action, entity_type, entity_id, reason, metadata_json, occurred_at, event_version) \
                 VALUES (?, ?, NULL, ?, ?, ?, 'credential.updated', 'local_credential', NULL, NULL, '{\"fields\":[\"state\"]}', ?, 1)",
            )
            .bind(id_value)
            .bind(ORG)
            .bind(actor)
            .bind(user)
            .bind(session)
            .bind(LATER)
            .execute(&pool)
            .await
        }
    };

    insert(
        "00000000-0000-0000-0000-0000000000f0",
        "USER",
        Some(USER),
        Some(SESSION),
    )
    .await?;
    insert(
        "00000000-0000-0000-0000-0000000000f1",
        "USER",
        Some(USER),
        None,
    )
    .await?;
    insert("00000000-0000-0000-0000-0000000000f2", "SYSTEM", None, None).await?;
    assert!(
        insert(
            "00000000-0000-0000-0000-0000000000f3",
            "SYSTEM",
            Some(USER),
            None
        )
        .await
        .is_err()
    );
    assert!(
        insert(
            "00000000-0000-0000-0000-0000000000f4",
            "SYSTEM",
            None,
            Some(SESSION)
        )
        .await
        .is_err()
    );
    assert!(
        insert(
            "00000000-0000-0000-0000-0000000000f5",
            "USER",
            Some(USER_TWO),
            Some(SESSION)
        )
        .await
        .is_err(),
        "sesión de otro usuario debe rechazarse"
    );

    // Metadata fuera de límite.
    assert!(
        sqlx::query(
            "INSERT INTO audit_event (id, organization_id, actor_type, actor_user_id, action, entity_type, metadata_json, occurred_at, event_version) \
             VALUES ('00000000-0000-0000-0000-0000000000f6', ?, 'SYSTEM', NULL, 'x', 'y', ?, ?, 1)",
        )
        .bind(ORG)
        .bind(format!("{{\"blob\":\"{}\"}}", "a".repeat(16_500)))
        .bind(LATER)
        .execute(&pool)
        .await
        .is_err()
    );
    // event_version 0 rechazado.
    assert!(
        sqlx::query(
            "INSERT INTO audit_event (id, organization_id, actor_type, action, entity_type, occurred_at, event_version) \
             VALUES ('00000000-0000-0000-0000-0000000000f7', ?, 'SYSTEM', 'x', 'y', ?, 0)",
        )
        .bind(ORG)
        .bind(LATER)
        .execute(&pool)
        .await
        .is_err()
    );

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
    Ok(())
}

#[tokio::test]
async fn integrity_errors_roll_back_and_survive_reopen() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("integrity.sqlite");
    let pool = open_proposed(&path).await?;
    seed(&pool).await?;

    // CAS sobre credencial.
    let stale = sqlx::query(
        "UPDATE local_credential SET failed_login_attempts = failed_login_attempts + 1, \
         credential_version = credential_version + 1 WHERE id = ? AND credential_version = 99",
    )
    .bind(CREDENTIAL)
    .execute(&pool)
    .await?;
    assert_eq!(stale.rows_affected(), 0);

    // Rollback por error de integridad dentro de una transacción.
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    sqlx::query(
        "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000000a5', ?, 'Temp', 'temp@prop.test', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    let invalid = sqlx::query(
        "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000000a6', ?, 'Temp', 'bad', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await;
    assert!(invalid.is_err());
    transaction.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pos_user WHERE id = '00000000-0000-0000-0000-0000000000a5'"
        )
        .fetch_one(&pool)
        .await?,
        0
    );

    pool.close().await;
    let reopened = open(&path, Duration::from_millis(75)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_credential")
            .fetch_one(&reopened)
            .await?,
        1
    );
    sqlx::query(
        "INSERT INTO audit_event (id, organization_id, actor_type, action, entity_type, occurred_at, event_version) \
         VALUES ('00000000-0000-0000-0000-0000000000fa', ?, 'SYSTEM', 'x', 'y', ?, 1)",
    )
    .bind(ORG)
    .bind(LATER)
    .execute(&reopened)
    .await?;
    assert!(
        sqlx::query("UPDATE audit_event SET action = 'tampered'")
            .execute(&reopened)
            .await
            .is_err(),
        "el trigger append-only sobrevive a la reapertura"
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn installation_identity_matches_the_canonical_section()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("installation.sqlite")).await?;
    seed(&pool).await?;

    let row = sqlx::query_as::<_, (String, i64, String, String)>(
        "SELECT organization_id, generation, status, public_key_fingerprint \
         FROM installation_identity WHERE id = ?",
    )
    .bind(INSTALLATION)
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, ORG);
    assert_eq!(row.1, 1);
    assert_eq!(row.2, "ACTIVE");
    assert_eq!(row.3, "ed25519:test-fingerprint-a");

    let insert = |id_value: &'static str, owner: &'static str, generation: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO installation_identity (id, organization_id, generation, public_key_fingerprint, initialized_at, updated_at) \
                 VALUES (?, ?, ?, 'ed25519:test', ?, ?)",
            )
            .bind(id_value)
            .bind(owner)
            .bind(generation)
            .bind(TIME)
            .bind(TIME)
            .execute(&pool)
            .await
        }
    };
    assert!(
        insert("00000000-0000-0000-0000-0000000000f5", ORG, 0)
            .await
            .is_err(),
        "generation debe ser >= 1"
    );
    assert!(
        insert("00000000-0000-0000-0000-0000000000F6", ORG, 1)
            .await
            .is_err(),
        "UUID en mayúsculas rechazado"
    );
    assert!(
        insert("00000000-0000-0000-0000-0000000000f7", ORG_TWO, 1)
            .await
            .is_ok(),
        "una organización existente es válida"
    );
    assert!(
        insert(
            "00000000-0000-0000-0000-0000000000f8",
            "00000000-0000-0000-0000-000000000099",
            1
        )
        .await
        .is_err(),
        "organización inexistente rechazada"
    );
    assert!(
        sqlx::query("INSERT INTO installation_identity (id, organization_id, initialized_at, updated_at) VALUES ('00000000-0000-0000-0000-0000000000f9', ?, ?, ?)")
            .bind(ORG)
            .bind(TIME)
            .bind(TIME)
            .execute(&pool)
            .await
            .is_err(),
        "public_key_fingerprint es obligatoria"
    );
    Ok(())
}

#[tokio::test]
async fn permission_grammar_matches_rust_validation() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("permission-grammar.sqlite")).await?;

    let invalid = [
        ".organization.read",
        "organization..read",
        "organization.read.",
        "Organization.read",
        "organization read",
        "organization.*",
        "organization/read",
    ];
    for code in invalid {
        assert!(
            pos_core::identity::PermissionCode::new(code).is_err(),
            "Rust aceptó {code}"
        );
        let inserted = sqlx::query("INSERT INTO permission (code, description) VALUES (?, 'x')")
            .bind(code)
            .execute(&pool)
            .await;
        assert!(inserted.is_err(), "SQLite aceptó {code}");
    }
    assert!(
        sqlx::query(
            "INSERT INTO permission (code, description) VALUES ('cash-register.read', 'x')"
        )
        .execute(&pool)
        .await
        .is_err(),
        "el código ya existe en el seed"
    );
    Ok(())
}

/// El contrato de `AuditOccurredAt` admite fracción opcional de segundo.
/// El CHECK candidato debe aceptar ambas formas sin habilitar texto libre.
#[tokio::test]
async fn audit_occurred_at_accepts_optional_fraction_and_rejects_junk()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("audit-timestamp.sqlite")).await?;
    seed(&pool).await?;

    let mut accepted = 0_u16;
    for stamp in [
        "2026-10-29T14:35:12Z",
        "2026-10-29T14:35:12.1Z",
        "2026-10-29T14:35:12.123Z",
        "2026-10-29T14:35:12.123456789Z",
    ] {
        accepted += 1;
        let id = format!("00000000-0000-0000-0000-0000000000c{accepted}");
        sqlx::query(
            "INSERT INTO audit_event (id, organization_id, actor_type, action, entity_type, occurred_at, event_version) \
             VALUES (?, ?, 'SYSTEM', 'timestamp.test', 'audit_event', ?, 1)",
        )
        .bind(id)
        .bind(ORG)
        .bind(stamp)
        .execute(&pool)
        .await
        .unwrap_or_else(|error| panic!("instante aceptado indebidamente rechazado ({stamp}): {error}"));
    }
    assert_eq!(accepted, 4);

    for (index, stamp) in [
        "2026-10-29T14:35:12",
        "2026-10-29T14:35:12.Z",
        "2026-10-29T14:35:12.12aZ",
        "2026-10-29 14:35:12Z",
        "2026-10-29T14:35:12+00:00",
        "2026-10-29T14:35:12.123",
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("00000000-0000-0000-0000-0000000001{index:02x}");
        assert!(
            sqlx::query(
                "INSERT INTO audit_event (id, organization_id, actor_type, action, entity_type, occurred_at, event_version) \
                 VALUES (?, ?, 'SYSTEM', 'timestamp.test', 'audit_event', ?, 1)",
            )
            .bind(id)
            .bind(ORG)
            .bind(stamp)
            .execute(&pool)
            .await
            .is_err(),
            "instante aceptado indebidamente: {stamp}"
        );
    }
    Ok(())
}

/// `pos-local::migrate()` solo descubre `migrations/local`: las propuestas
/// candidatas viven fuera y no pueden ejecutarse por accidente.
#[tokio::test]
async fn productive_migrator_does_not_discover_candidate_proposals()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("activation.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await?,
        1,
        "solo 0001 está aplicada en el directorio productivo"
    );
    for table in [
        "pos_user",
        "local_session",
        "local_credential",
        "role",
        "permission",
        "branch_membership",
        "audit_event",
        "bootstrap_record",
        "installation_identity",
    ] {
        let present = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(&pool)
        .await?;
        assert_eq!(
            present, 0,
            "la tabla candidata {table} no existe en producto"
        );
    }
    Ok(())
}

/// El primer arranque exige el orden organización → instalación → bootstrap.
///
/// `installation_identity.organization_id` es obligatorio en F1–10 §16.3, por lo
/// que no existe dependencia circular: la organización inicial se crea en la
/// misma transacción, antes de la instalación. `bootstrap_record` permanece en
/// `PENDING` sin organización (la FK compuesta no se evalúa con `NULL`) y solo
/// pasa a `COMPLETED` cuando instalación, organización, administrador y rol son
/// coherentes.
#[tokio::test]
async fn first_run_requires_organization_before_installation_and_commits_once()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("first-run.sqlite");
    let pool = open_proposed(&path).await?;

    // Sin organización no puede existir instalación: el owner es obligatorio.
    let orphan = sqlx::query(
        "INSERT INTO installation_identity (id, organization_id, public_key_fingerprint, initialized_at, updated_at) \
         VALUES (?, ?, 'ed25519:synthetic-fingerprint', ?, ?)",
    )
    .bind(INSTALLATION)
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await;
    assert!(
        orphan.is_err(),
        "la instalación no puede preceder a su organización"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM installation_identity")
            .fetch_one(&pool)
            .await?,
        0,
        "el intento rechazado no deja filas"
    );

    // Primer arranque completo en un único commit.
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    sqlx::query(
        "INSERT INTO organization (id, code, name, currency, timezone, locale, stock_negative_policy, status, version, created_at, updated_at) \
         VALUES (?, 'first-run', 'Instalación Inicial', 'MXN', 'UTC', 'es-MX', 'STRICT', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO installation_identity (id, organization_id, public_key_fingerprint, initialized_at, updated_at) \
         VALUES (?, ?, 'ed25519:synthetic-fingerprint', ?, ?)",
    )
    .bind(INSTALLATION)
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO bootstrap_record (installation_id, state, updated_at) VALUES (?, 'PENDING', ?)",
    )
    .bind(INSTALLATION)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
         VALUES (?, ?, 'Administrador', 'admin@first-run.test', 'ACTIVE', 0, ?, ?)",
    )
    .bind(USER)
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
         VALUES (?, ?, 'administrator', 'Administrador', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ROLE_ORG)
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "UPDATE bootstrap_record SET state = 'COMPLETED', completed_at = ?, organization_id = ?, \
         administrator_user_id = ?, administrator_role_id = ?, bootstrap_version = 1, updated_at = ? \
         WHERE installation_id = ? AND state = 'PENDING'",
    )
    .bind(TIME)
    .bind(ORG)
    .bind(USER)
    .bind(ROLE_ORG)
    .bind(TIME)
    .bind(INSTALLATION)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO audit_event (id, organization_id, actor_type, actor_user_id, session_id, action, entity_type, entity_id, occurred_at, event_version) \
         VALUES ('00000000-0000-0000-0000-0000000000f7', ?, 'SYSTEM', NULL, NULL, 'bootstrap.completed', 'bootstrap_record', ?, ?, 1)",
    )
    .bind(ORG)
    .bind(INSTALLATION)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    transaction.commit().await?;

    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM bootstrap_record WHERE installation_id = ?"
        )
        .bind(INSTALLATION)
        .fetch_one(&pool)
        .await?,
        "COMPLETED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT organization_id FROM bootstrap_record WHERE installation_id = ?"
        )
        .bind(INSTALLATION)
        .fetch_one(&pool)
        .await?,
        ORG
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT actor_type FROM audit_event WHERE action = 'bootstrap.completed'"
        )
        .fetch_one(&pool)
        .await?,
        "SYSTEM"
    );

    pool.close().await;
    let reopened = open(&path, Duration::from_millis(75)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM bootstrap_record WHERE state = 'COMPLETED'"
        )
        .fetch_one(&reopened)
        .await?,
        1
    );
    reopened.close().await;
    Ok(())
}

/// Un fallo de auditoría revierte organización, instalación y bootstrap juntos.
#[tokio::test]
async fn first_run_rolls_back_everything_when_audit_fails() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let pool = open_proposed(&temp.path().join("first-run-rollback.sqlite")).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    sqlx::query(
        "INSERT INTO organization (id, code, name, currency, timezone, locale, stock_negative_policy, status, version, created_at, updated_at) \
         VALUES (?, 'first-run', 'Instalación Inicial', 'MXN', 'UTC', 'es-MX', 'STRICT', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO installation_identity (id, organization_id, public_key_fingerprint, initialized_at, updated_at) \
         VALUES (?, ?, 'ed25519:synthetic-fingerprint', ?, ?)",
    )
    .bind(INSTALLATION)
    .bind(ORG)
    .bind(TIME)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO bootstrap_record (installation_id, state, updated_at) VALUES (?, 'PENDING', ?)",
    )
    .bind(INSTALLATION)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await?;
    // SYSTEM con usuario atribuido viola el contrato de actor: debe fallar.
    let audit = sqlx::query(
        "INSERT INTO audit_event (id, organization_id, actor_type, actor_user_id, session_id, action, entity_type, occurred_at, event_version) \
         VALUES ('00000000-0000-0000-0000-0000000000f6', ?, 'SYSTEM', ?, NULL, 'bootstrap.completed', 'bootstrap_record', ?, 1)",
    )
    .bind(ORG)
    .bind(USER)
    .bind(TIME)
    .execute(&mut *transaction.connection())
    .await;
    assert!(audit.is_err(), "SYSTEM no admite actor_user_id");
    transaction.rollback().await?;

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM installation_identity")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bootstrap_record")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn bootstrap_authorization_models_preauthorization_and_append_only_events()
-> Result<(), Box<dyn std::error::Error>> {
    const PROVISIONAL: &str = "00000000-0000-0000-0000-0000000001b0";
    const PROVISIONAL_TWO: &str = "00000000-0000-0000-0000-0000000001b1";
    const AUTH: &str = "00000000-0000-0000-0000-0000000001a0";
    const AUTH_REISSUED: &str = "00000000-0000-0000-0000-0000000001a2";
    const EVENT_ISSUED: &str = "00000000-0000-0000-0000-0000000001e0";
    const EVENT_REJECTED: &str = "00000000-0000-0000-0000-0000000001e1";

    let temp = tempfile::tempdir()?;
    let path = temp.path().join("authorization.sqlite");
    let pool = open_proposed(&path).await?;
    seed(&pool).await?;

    // La preautorización existe antes de la organización y no depende de ella.
    for (id, fingerprint) in [
        (PROVISIONAL, "ed25519:provisional-a"),
        (PROVISIONAL_TWO, "ed25519:provisional-b"),
    ] {
        sqlx::query(
            "INSERT INTO provisional_installation (id, fingerprint, created_at) \
             VALUES (?, ?, 900)",
        )
        .bind(id)
        .bind(fingerprint)
        .execute(&pool)
        .await?;
    }

    sqlx::query(
        "INSERT INTO bootstrap_authorization \
         (id, installation_id, purpose, token_digest, issued_at, expires_at, updated_at) \
         VALUES (?, ?, 'BOOTSTRAP', ?, 1000, 1600, 1000)",
    )
    .bind(AUTH)
    .bind(PROVISIONAL)
    .bind("a".repeat(64))
    .execute(&pool)
    .await?;

    // Un digest identifica una sola autorización.
    assert!(
        sqlx::query(
            "INSERT INTO bootstrap_authorization \
             (id, installation_id, purpose, token_digest, issued_at, expires_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000001a1', ?, 'BOOTSTRAP', ?, 1000, 1600, 1000)",
        )
        .bind(PROVISIONAL_TWO)
        .bind("a".repeat(64))
        .execute(&pool)
        .await
        .is_err()
    );

    // Un solo derecho vivo por instalación y finalidad.
    assert!(
        sqlx::query(
            "INSERT INTO bootstrap_authorization \
             (id, installation_id, purpose, token_digest, issued_at, expires_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000001a3', ?, 'BOOTSTRAP', ?, 1000, 1600, 1000)",
        )
        .bind(PROVISIONAL)
        .bind("b".repeat(64))
        .execute(&pool)
        .await
        .is_err()
    );

    // Ventana fuera de política.
    assert!(
        sqlx::query(
            "INSERT INTO bootstrap_authorization \
             (id, installation_id, purpose, token_digest, issued_at, expires_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000001a4', ?, \
                     'ADMINISTRATIVE_RECOVERY', ?, 1000, 1050, 1000)",
        )
        .bind(PROVISIONAL)
        .bind("c".repeat(64))
        .execute(&pool)
        .await
        .is_err()
    );

    // PENDING no admite cierre y EXPIRED no cierra antes de caducar.
    assert!(
        sqlx::query(
            "INSERT INTO bootstrap_authorization \
             (id, installation_id, purpose, token_digest, issued_at, expires_at, state, \
              closed_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000001a5', ?, \
                     'ADMINISTRATIVE_RECOVERY', ?, 1000, 1600, 'PENDING', 1200, 1200)",
        )
        .bind(PROVISIONAL)
        .bind("d".repeat(64))
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "INSERT INTO bootstrap_authorization \
             (id, installation_id, purpose, token_digest, issued_at, expires_at, state, \
              closed_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-0000000001a6', ?, \
                     'ADMINISTRATIVE_RECOVERY', ?, 1000, 1600, 'EXPIRED', 1500, 1500)",
        )
        .bind(PROVISIONAL)
        .bind("e".repeat(64))
        .execute(&pool)
        .await
        .is_err()
    );

    // Eventos auditables sin secretos y append-only.
    for (id, kind, context) in [
        (EVENT_ISSUED, "ISSUED", Some("issuer=native")),
        (EVENT_REJECTED, "REJECTED", Some("reason=expired")),
    ] {
        sqlx::query(
            "INSERT INTO bootstrap_authorization_event \
             (id, authorization_id, event_type, occurred_at, security_context) \
             VALUES (?, ?, ?, 1000, ?)",
        )
        .bind(id)
        .bind(AUTH)
        .bind(kind)
        .bind(context)
        .execute(&pool)
        .await?;
    }
    assert!(
        sqlx::query("UPDATE bootstrap_authorization_event SET event_type = 'CONSUMED'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM bootstrap_authorization_event")
            .execute(&pool)
            .await
            .is_err()
    );

    // Consumo con CAS: solo la primera transición afecta una fila.
    let consumed = sqlx::query(
        "UPDATE bootstrap_authorization SET state = 'CONSUMED', closed_at = 1200, \
         updated_at = 1200 WHERE id = ? AND state = 'PENDING'",
    )
    .bind(AUTH)
    .execute(&pool)
    .await?;
    assert_eq!(consumed.rows_affected(), 1);
    let replay = sqlx::query(
        "UPDATE bootstrap_authorization SET state = 'CONSUMED', closed_at = 1201, \
         updated_at = 1201 WHERE id = ? AND state = 'PENDING'",
    )
    .bind(AUTH)
    .execute(&pool)
    .await?;
    assert_eq!(replay.rows_affected(), 0);

    // Tras el estado terminal se puede reemitir para la misma instalación.
    sqlx::query(
        "INSERT INTO bootstrap_authorization \
         (id, installation_id, purpose, token_digest, issued_at, expires_at, updated_at) \
         VALUES (?, ?, 'BOOTSTRAP', ?, 1300, 1900, 1300)",
    )
    .bind(AUTH_REISSUED)
    .bind(PROVISIONAL)
    .bind("f".repeat(64))
    .execute(&pool)
    .await?;

    // Reclamación de la preautorización: ambos campos o ninguno.
    assert!(
        sqlx::query("UPDATE provisional_installation SET claimed_at = 1700 WHERE id = ?")
            .bind(PROVISIONAL)
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE provisional_installation SET claimed_at = 1700, claimed_installation_id = ? \
         WHERE id = ?",
    )
    .bind(INSTALLATION)
    .bind(PROVISIONAL)
    .execute(&pool)
    .await?;

    pool.close().await;

    // Durabilidad tras reapertura: los estados y el append-only se conservan.
    let reopened = open(&path, Duration::from_millis(75)).await?;
    let state: String =
        sqlx::query_scalar("SELECT state FROM bootstrap_authorization WHERE id = ?")
            .bind(AUTH)
            .fetch_one(&reopened)
            .await?;
    assert_eq!(state, "CONSUMED");
    let claimed: i64 = sqlx::query_scalar(
        "SELECT claimed_installation_id IS NOT NULL FROM provisional_installation WHERE id = ?",
    )
    .bind(PROVISIONAL)
    .fetch_one(&reopened)
    .await?;
    assert_eq!(claimed, 1);
    assert!(
        sqlx::query("DELETE FROM bootstrap_authorization_event")
            .execute(&reopened)
            .await
            .is_err()
    );
    reopened.close().await;
    Ok(())
}
