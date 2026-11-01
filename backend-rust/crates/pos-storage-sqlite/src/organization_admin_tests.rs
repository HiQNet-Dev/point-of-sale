//! Configuración del negocio sobre SQLite productiva.
//!
//! Demuestra de extremo a extremo la composición: autorización DB-authoritative,
//! UoW con `BEGIN IMMEDIATE`, CAS optimista, auditoría canónica y reapertura.
//! Las filas se siembran con SQL directo; los casos de uso son los del producto.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_application::organization::administration::{
    BranchConfigurationUpdate, BranchCreation, OrganizationConfigurationUpdate, TimezoneUpdate,
};
use pos_core::{
    audit::AuditOccurredAt,
    identity::{LocalPrincipalService, SessionHash, UtcSeconds, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        IsoCurrency, LocaleTag, NegativeStockPolicy, NewBranch, NewOrganization, Organization,
        Status, Timezone,
    },
};
use sqlx::SqlitePool;

use super::*;
use crate::{
    identity::SqliteIdentityLab, organization_admin::SqliteOrganizationAdministration,
    organization_branch::SqliteOrganizationBranchStore,
};

const ORG_A: &str = "00000000-0000-0000-0000-00000000e0a0";
const ORG_B: &str = "00000000-0000-0000-0000-00000000e0a1";
const USER_A: &str = "00000000-0000-0000-0000-00000000e0b0";
const USER_B: &str = "00000000-0000-0000-0000-00000000e0b1";
const ADMIN_ROLE: &str = "00000000-0000-0000-0000-00000000e0c0";
const READER_ROLE: &str = "00000000-0000-0000-0000-00000000e0c1";
const SESSION_A: &str = "00000000-0000-0000-0000-00000000e0d0";
const SESSION_B: &str = "00000000-0000-0000-0000-00000000e0d1";
const BRANCH_A: &str = "00000000-0000-0000-0000-00000000e0e0";
const BRANCH_EXTRA: &str = "00000000-0000-0000-0000-00000000e0e1";
const MEMBERSHIP_A: &str = "00000000-0000-0000-0000-00000000e0f0";
const MEMBERSHIP_EXTRA: &str = "00000000-0000-0000-0000-00000000e0f1";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const HASH_A: &str = "1111111122222222333333334444444455555555666666667777777788888888";
const HASH_B: &str = "2222222233333333444444445555555566666666777777778888888899999999";
const NOW: i64 = 1_793_235_600;

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn utc(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("instante: {error}"))
}

fn at(value: &str) -> AuditOccurredAt {
    AuditOccurredAt::new(value).unwrap_or_else(|error| panic!("instante: {error}"))
}

fn organization(value: &str, code: &str) -> Organization {
    Organization::create(
        id(value),
        NewOrganization {
            name: DisplayName::new("Negocio").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("America/Mexico_City")
                .unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    )
}

fn branch(value: &str, owner: &str, code: &str, timezone: Option<&str>) -> Branch {
    Branch::create(
        id(value),
        id(owner),
        NewBranch {
            name: DisplayName::new("Sucursal Centro")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone_override: timezone
                .map(|value| Timezone::new(value).unwrap_or_else(|error| panic!("tz: {error}"))),
        },
    )
}

/// Generador determinista que nunca repite identidades.
struct FixtureIds(u32);

impl IdentityGenerator for FixtureIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        self.0 += 1;
        EntityId::parse(&format!("00000000-0000-0000-0000-{:012x}", 0x900 + self.0))
            .map_err(|_| ApplicationError::Storage)
    }
}

async fn candidate() -> Result<(tempfile::TempDir, SqlitePool), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("business.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    store
        .insert_organization(&organization(ORG_A, "negocio-a"), TIME, TIME)
        .await?;
    store
        .insert_organization(&organization(ORG_B, "negocio-b"), TIME, TIME)
        .await?;
    store
        .insert_branch(&branch(BRANCH_A, ORG_A, "centro", None), TIME, TIME)
        .await?;
    seed_security(&pool).await?;
    Ok((temp, pool))
}

/// Siembra usuarios, roles, permisos, sesiones y membresías de sucursal.
async fn seed_security(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for (user, owner, email) in [
        (USER_A, ORG_A, "negocio-a@hiqnet.test"),
        (USER_B, ORG_B, "negocio-b@hiqnet.test"),
    ] {
        sqlx::query(
            "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES (?, ?, 'Administradora', ?, 'ACTIVE', 0, ?, ?)",
        )
        .bind(user)
        .bind(owner)
        .bind(email)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    for (role, owner, code) in [
        (ADMIN_ROLE, ORG_A, "negocio-admin"),
        (READER_ROLE, ORG_B, "negocio-lector"),
    ] {
        sqlx::query(
            "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, 'Rol', 'ACTIVE', 0, ?, ?)",
        )
        .bind(role)
        .bind(owner)
        .bind(code)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    // A administra el negocio; B solo lee catálogo, así que se prueba
    // deny-by-default sin conceder de más.
    for permission in [
        "organization.read",
        "organization.update",
        "branch.read",
        "branch.update",
        "branch.create",
    ] {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(ADMIN_ROLE)
            .bind(permission)
            .execute(pool)
            .await?;
    }
    sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
        .bind(READER_ROLE)
        .bind("product.read")
        .execute(pool)
        .await?;
    for (owner, user, role) in [(ORG_A, USER_A, ADMIN_ROLE), (ORG_B, USER_B, READER_ROLE)] {
        sqlx::query(
            "INSERT INTO user_organization_role (organization_id, user_id, role_id, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(owner)
        .bind(user)
        .bind(role)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    for (session, user, hash) in [(SESSION_A, USER_A, HASH_A), (SESSION_B, USER_B, HASH_B)] {
        sqlx::query(
            "INSERT INTO local_session (id, user_id, session_hash, state, created_at, expires_at, revoked_at, session_version) \
             VALUES (?, ?, ?, 'ACTIVE', ?, ?, NULL, 1)",
        )
        .bind(session)
        .bind(user)
        .bind(hash)
        .bind(TIME)
        .bind(LATER)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO branch_membership (id, organization_id, branch_id, user_id, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'ACTIVE', 0, ?, ?)",
    )
    .bind(MEMBERSHIP_A)
    .bind(ORG_A)
    .bind(BRANCH_A)
    .bind(USER_A)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO branch_membership_role (organization_id, membership_id, role_id, created_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(MEMBERSHIP_A)
    .bind(ADMIN_ROLE)
    .bind(TIME)
    .execute(pool)
    .await?;
    Ok(())
}

async fn principal(
    pool: &SqlitePool,
    hash: &str,
    branch: Option<&EntityId>,
) -> Result<VerifiedPrincipal, Box<dyn std::error::Error>> {
    let session_hash = SessionHash::new(hash)?;
    let mut connection = pool.acquire().await?;
    let result = {
        let mut service = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection));
        service.authenticate(&session_hash, utc(NOW), branch).await
    };
    connection.close().await?;
    Ok(result?)
}

fn application(pool: &SqlitePool) -> SqliteOrganizationAdministration {
    SqliteOrganizationAdministration::new(SqliteUnitOfWork::new(pool.clone()))
}

async fn audit_events(pool: &SqlitePool, action: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT metadata_json FROM audit_event WHERE action = ? ORDER BY occurred_at",
    )
    .bind(action)
    .fetch_all(pool)
    .await
}

#[tokio::test]
async fn reads_the_organization_of_the_principal() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let org = application(&pool).get_organization(&owner).await?;
    assert_eq!(org.id(), &id(ORG_A));
    assert_eq!(org.code().as_str(), "negocio-a");
    assert_eq!(org.currency().as_str(), "MXN");
    assert_eq!(org.status(), Status::Active);
    assert_eq!(org.negative_stock_policy(), NegativeStockPolicy::Strict);
    assert_eq!(org.revision(), 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn updates_the_editable_configuration_with_cas_and_audit()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    let updated = app
        .update_organization(
            &owner,
            &mut identities,
            OrganizationConfigurationUpdate {
                expected_revision: 0,
                name: Some("Café Central".to_owned()),
                timezone: Some("America/Tijuana".to_owned()),
                locale: Some("es-MX".to_owned()),
                negative_stock_policy: Some("ALLOW_NEGATIVE".to_owned()),
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(updated.name().as_str(), "Café Central");
    assert_eq!(updated.timezone().as_str(), "America/Tijuana");
    assert_eq!(
        updated.negative_stock_policy(),
        NegativeStockPolicy::AllowNegative
    );
    assert_eq!(updated.revision(), 1);
    // La moneda y el estado no forman parte del contrato de entrada.
    assert_eq!(updated.currency().as_str(), "MXN");
    assert_eq!(updated.status(), Status::Active);

    let events = audit_events(&pool, "organization.updated").await?;
    assert_eq!(events.len(), 1);
    let metadata = events[0].clone();
    for field in ["name", "timezone", "inventoryNegativeStockPolicy"] {
        assert!(metadata.contains(field), "falta {field} en {metadata}");
    }

    pool.close().await;
    let reopened = open(&pool_file(&_temp), Duration::from_millis(75)).await?;
    let owner = principal(&reopened, HASH_A, None).await?;
    let persisted = application(&reopened).get_organization(&owner).await?;
    assert_eq!(persisted.name().as_str(), "Café Central");
    assert_eq!(persisted.revision(), 1);
    reopened.close().await;
    Ok(())
}

fn pool_file(temp: &tempfile::TempDir) -> std::path::PathBuf {
    temp.path().join("business.sqlite")
}

#[tokio::test]
async fn a_no_op_update_keeps_the_revision_and_writes_no_audit()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    let unchanged = app
        .update_organization(
            &owner,
            &mut identities,
            OrganizationConfigurationUpdate {
                expected_revision: 0,
                name: Some("Negocio".to_owned()),
                timezone: None,
                locale: None,
                negative_stock_policy: None,
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(unchanged.revision(), 0);
    assert_eq!(audit_events(&pool, "organization.updated").await?.len(), 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn a_stale_revision_is_a_conflict() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    let error = app
        .update_organization(
            &owner,
            &mut identities,
            OrganizationConfigurationUpdate {
                expected_revision: 7,
                name: Some("Otro".to_owned()),
                timezone: None,
                locale: None,
                negative_stock_policy: None,
            },
            &at(TIME),
        )
        .await
        .err();
    assert_eq!(error, Some(ApplicationError::ConcurrentModification));
    let reread = app.get_organization(&owner).await?;
    assert_eq!(reread.revision(), 0);
    assert_eq!(reread.name().as_str(), "Negocio");
    assert_eq!(audit_events(&pool, "organization.updated").await?.len(), 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn invalid_configuration_is_rejected_without_writing()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    for (name, timezone, locale, policy) in [
        (Some("".to_owned()), None, None, None),
        (None, Some("Marte/Olympus".to_owned()), None, None),
        (None, None, Some("es_XX_ZZ".to_owned()), None),
        (None, None, None, Some("SOMETIMES".to_owned())),
    ] {
        let error = app
            .update_organization(
                &owner,
                &mut identities,
                OrganizationConfigurationUpdate {
                    expected_revision: 0,
                    name,
                    timezone,
                    locale,
                    negative_stock_policy: policy,
                },
                &at(TIME),
            )
            .await
            .err();
        assert_eq!(error, Some(ApplicationError::InvalidOrganizationInput));
    }
    assert_eq!(app.get_organization(&owner).await?.revision(), 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn deny_by_default_for_a_principal_without_business_permissions()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let reader = principal(&pool, HASH_B, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    assert_eq!(
        app.get_organization(&reader).await.err(),
        Some(ApplicationError::Forbidden)
    );
    assert_eq!(
        app.update_organization(
            &reader,
            &mut identities,
            OrganizationConfigurationUpdate {
                expected_revision: 0,
                name: Some("Intruso".to_owned()),
                timezone: None,
                locale: None,
                negative_stock_policy: None,
            },
            &at(TIME),
        )
        .await
        .err(),
        Some(ApplicationError::Forbidden)
    );
    assert_eq!(
        app.list_branches(&reader).await.err(),
        Some(ApplicationError::Forbidden)
    );
    assert_eq!(
        app.get_branch(&reader, &id(BRANCH_A)).await.err(),
        Some(ApplicationError::Forbidden)
    );
    assert_eq!(
        app.create_branch(
            &reader,
            &mut identities,
            BranchCreation {
                code: "norte".to_owned(),
                name: "Norte".to_owned(),
                timezone: None,
            },
            &at(TIME),
        )
        .await
        .err(),
        Some(ApplicationError::Forbidden)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn lists_branches_with_the_effective_timezone() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let branches = app.list_branches(&owner).await?;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch.id(), &id(BRANCH_A));
    assert!(!branches[0].overrides_timezone());
    // Hereda la zona horaria de la organización.
    assert_eq!(
        branches[0].effective_timezone.as_str(),
        "America/Mexico_City"
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn branch_timezone_tristate_and_no_op() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);

    // Set: define una zona propia.
    let set = app
        .update_branch(
            &owner,
            &mut identities,
            &id(BRANCH_A),
            BranchConfigurationUpdate {
                expected_revision: 0,
                name: None,
                timezone: TimezoneUpdate::Set("America/Cancun".to_owned()),
            },
            &at(TIME),
        )
        .await?;
    assert!(set.overrides_timezone());
    assert_eq!(set.effective_timezone.as_str(), "America/Cancun");
    assert_eq!(set.branch.revision(), 1);

    // Keep: conserva la configuración y no audita.
    let keep = app
        .update_branch(
            &owner,
            &mut identities,
            &id(BRANCH_A),
            BranchConfigurationUpdate {
                expected_revision: 1,
                name: None,
                timezone: TimezoneUpdate::Keep,
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(keep.branch.revision(), 1);
    assert_eq!(audit_events(&pool, "branch.updated").await?.len(), 1);

    // Clear: vuelve a heredar.
    let clear = app
        .update_branch(
            &owner,
            &mut identities,
            &id(BRANCH_A),
            BranchConfigurationUpdate {
                expected_revision: 1,
                name: Some("Sucursal Norte".to_owned()),
                timezone: TimezoneUpdate::Clear,
            },
            &at(TIME),
        )
        .await?;
    assert!(!clear.overrides_timezone());
    assert_eq!(clear.effective_timezone.as_str(), "America/Mexico_City");
    assert_eq!(clear.branch.name().as_str(), "Sucursal Norte");
    assert_eq!(clear.branch.revision(), 2);

    let events = audit_events(&pool, "branch.updated").await?;
    assert_eq!(events.len(), 2);
    assert!(events[1].contains("timezone"));
    assert!(events[1].contains("name"));
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn branch_update_with_a_stale_revision_changes_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    let error = app
        .update_branch(
            &owner,
            &mut identities,
            &id(BRANCH_A),
            BranchConfigurationUpdate {
                expected_revision: 4,
                name: Some("Nombre".to_owned()),
                timezone: TimezoneUpdate::Keep,
            },
            &at(TIME),
        )
        .await
        .err();
    assert_eq!(error, Some(ApplicationError::ConcurrentModification));
    let branch = app.get_branch(&owner, &id(BRANCH_A)).await?;
    assert_eq!(branch.branch.revision(), 0);
    assert_eq!(branch.branch.name().as_str(), "Sucursal Centro");
    assert_eq!(audit_events(&pool, "branch.updated").await?.len(), 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn a_branch_of_another_organization_is_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    assert_eq!(
        app.get_branch(&owner, &id(BRANCH_EXTRA)).await.err(),
        Some(ApplicationError::BranchNotFound)
    );
    assert_eq!(
        app.update_branch(
            &owner,
            &mut identities,
            &id(BRANCH_EXTRA),
            BranchConfigurationUpdate {
                expected_revision: 0,
                name: Some("Ajeno".to_owned()),
                timezone: TimezoneUpdate::Keep,
            },
            &at(TIME),
        )
        .await
        .err(),
        Some(ApplicationError::BranchNotFound)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn a_branch_scoped_principal_can_only_edit_its_own_branch()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    // Segunda sucursal de la misma organización, con membresía del usuario A.
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    store
        .insert_branch(&branch(BRANCH_EXTRA, ORG_A, "norte", None), TIME, TIME)
        .await?;
    sqlx::query(
        "INSERT INTO branch_membership (id, organization_id, branch_id, user_id, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'ACTIVE', 0, ?, ?)",
    )
    .bind(MEMBERSHIP_EXTRA)
    .bind(ORG_A)
    .bind(BRANCH_EXTRA)
    .bind(USER_A)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO branch_membership_role (organization_id, membership_id, role_id, created_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(MEMBERSHIP_EXTRA)
    .bind(ADMIN_ROLE)
    .bind(TIME)
    .execute(&pool)
    .await?;

    let scoped = principal(&pool, HASH_A, Some(&id(BRANCH_A))).await?;
    assert_eq!(scoped.scope().branch_id(), Some(&id(BRANCH_A)));
    let app = application(&pool);
    let mut identities = FixtureIds(0);

    // Su propia sucursal: autorizado.
    app.update_branch(
        &scoped,
        &mut identities,
        &id(BRANCH_A),
        BranchConfigurationUpdate {
            expected_revision: 0,
            name: Some("Centro renovado".to_owned()),
            timezone: TimezoneUpdate::Keep,
        },
        &at(TIME),
    )
    .await?;

    // Otra sucursal de la misma organización: denegado.
    assert_eq!(
        app.update_branch(
            &scoped,
            &mut identities,
            &id(BRANCH_EXTRA),
            BranchConfigurationUpdate {
                expected_revision: 0,
                name: Some("Intruso".to_owned()),
                timezone: TimezoneUpdate::Keep,
            },
            &at(TIME),
        )
        .await
        .err(),
        Some(ApplicationError::Forbidden)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn free_local_capacity_blocks_a_second_branch() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);
    let error = app
        .create_branch(
            &owner,
            &mut identities,
            BranchCreation {
                code: "norte".to_owned(),
                name: "Sucursal Norte".to_owned(),
                timezone: None,
            },
            &at(TIME),
        )
        .await
        .err();
    assert_eq!(error, Some(ApplicationError::BranchCapacityReached));
    let branches = app.list_branches(&owner).await?;
    assert_eq!(branches.len(), 1);
    assert_eq!(audit_events(&pool, "branch.created").await?.len(), 0);
    pool.close().await;
    Ok(())
}
