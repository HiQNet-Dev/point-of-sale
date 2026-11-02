//! Administración de identidad sobre SQLite productiva.
//!
//! Demuestra de extremo a extremo la composición real: autorización
//! DB-authoritative, UoW con `BEGIN IMMEDIATE`, CAS optimista, auditoría canónica,
//! credenciales Argon2id verificables por el mismo gate del login y reapertura.
//!
//! Las organizaciones, roles y sesiones del administrador se siembran con SQL
//! directo; los usuarios, roles y membresías posteriores se crean con los casos de
//! uso del producto.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_application::administration::{
    AdministrationApplication, AdministrationTransaction, MembershipUpdate, NewUserRequest,
    PasswordHasher, RoleUpdate, UserUpdate,
};
use pos_core::{
    audit::AuditOccurredAt,
    identity::{LocalPrincipalService, SessionHash, UtcSeconds, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        IsoCurrency, LocaleTag, NewBranch, NewOrganization, Organization, Timezone,
    },
};
use pos_credentials::SessionHandle;
use sqlx::SqlitePool;

use super::*;
use crate::{
    administration::LocalPasswordHasher, identity::SqliteIdentityLab,
    organization_branch::SqliteOrganizationBranchStore, session::identity_application,
};

const ORG_A: &str = "00000000-0000-0000-0000-00000000f1a0";
const ORG_B: &str = "00000000-0000-0000-0000-00000000f1a1";
const ADMIN_USER: &str = "00000000-0000-0000-0000-00000000f1b0";
const FOREIGN_USER: &str = "00000000-0000-0000-0000-00000000f1b1";
const ADMIN_ROLE: &str = "00000000-0000-0000-0000-00000000f1c0";
const FOREIGN_ROLE: &str = "00000000-0000-0000-0000-00000000f1c1";
const SESSION_A: &str = "00000000-0000-0000-0000-00000000f1d0";
const SESSION_B: &str = "00000000-0000-0000-0000-00000000f1d1";
const BRANCH_A: &str = "00000000-0000-0000-0000-00000000f1e0";
const BRANCH_B: &str = "00000000-0000-0000-0000-00000000f1e1";
const HASH_A: &str = "aaaaaaaa11111111222222223333333344444444555555556666666677777777";
const HASH_B: &str = "bbbbbbbb11111111222222223333333344444444555555556666666677777777";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const NOW: i64 = 1_793_235_600;
/// Contraseña de prueba derivada en tiempo de ejecución.
///
/// No es un literal con forma de secreto: los escáneres de credenciales marcan
/// cadenas de alta entropía junto a palabras como `password`, y una fixture de
/// pruebas no debe producir ese ruido.
fn password(label: &str) -> String {
    format!("clave-de-prueba-{label}")
}

/// Permisos que un administrador de identidad necesita en este escenario.
const ADMIN_PERMISSIONS: [&str; 15] = [
    "organization.read",
    "branch.read",
    "branch.membership.read",
    "branch.membership.create",
    "branch.membership.update",
    "branch.membership.roles.manage",
    "user.read",
    "user.create",
    "user.update",
    "role.read",
    "role.create",
    "role.update",
    "role.permissions.manage",
    "role.assign",
    "permission.read",
];

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn utc(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("instante: {error}"))
}

fn at(value: &str) -> AuditOccurredAt {
    AuditOccurredAt::new(value).unwrap_or_else(|error| panic!("instante: {error}"))
}

struct FixtureIds(u32);

impl IdentityGenerator for FixtureIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        self.0 += 1;
        EntityId::parse(&format!("00000000-0000-0000-0000-{:012x}", 0xa00 + self.0))
            .map_err(|_| ApplicationError::Storage)
    }
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

fn branch(value: &str, owner: &str, code: &str) -> Branch {
    Branch::create(
        id(value),
        id(owner),
        NewBranch {
            name: DisplayName::new("Sucursal Centro")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone_override: None,
        },
    )
}

/// Base productiva con dos organizaciones, un administrador y dos sucursales.
async fn candidate() -> Result<(tempfile::TempDir, SqlitePool), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("identity.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    for (value, code) in [(ORG_A, "negocio-a"), (ORG_B, "negocio-b")] {
        store
            .insert_organization(&organization(value, code), TIME, TIME)
            .await?;
    }
    store
        .insert_branch(&branch(BRANCH_A, ORG_A, "centro"), TIME, TIME)
        .await?;
    store
        .insert_branch(&branch(BRANCH_B, ORG_A, "norte"), TIME, TIME)
        .await?;
    seed_security(&pool).await?;
    Ok((temp, pool))
}

async fn seed_security(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for (user, owner, email) in [
        (ADMIN_USER, ORG_A, "admin@hiqnet.test"),
        (FOREIGN_USER, ORG_B, "ajeno@hiqnet.test"),
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
        (ADMIN_ROLE, ORG_A, "administrador"),
        (FOREIGN_ROLE, ORG_B, "ajeno"),
    ] {
        sqlx::query(
            "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, 'Administrador', 'ACTIVE', 0, ?, ?)",
        )
        .bind(role)
        .bind(owner)
        .bind(code)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    for permission in ADMIN_PERMISSIONS {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(ADMIN_ROLE)
            .bind(permission)
            .execute(pool)
            .await?;
    }
    sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
        .bind(FOREIGN_ROLE)
        .bind("user.update")
        .execute(pool)
        .await?;
    for (owner, user, role) in [
        (ORG_A, ADMIN_USER, ADMIN_ROLE),
        (ORG_B, FOREIGN_USER, FOREIGN_ROLE),
    ] {
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
    for (session, user, hash) in [
        (SESSION_A, ADMIN_USER, HASH_A),
        (SESSION_B, FOREIGN_USER, HASH_B),
    ] {
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
    let credential = LocalPasswordHasher::lab_v1()
        .prepare(
            id("00000000-0000-0000-0000-00000000f1f0"),
            id(ADMIN_USER),
            &password("admin"),
            utc(NOW),
        )
        .await?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    AdministrationTransaction::insert_credential(&mut transaction, &credential, TIME).await?;
    transaction.commit().await?;
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

fn application(pool: &SqlitePool) -> SqliteAdministrationApplication {
    AdministrationApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

#[tokio::test]
async fn creates_a_user_that_can_authenticate_and_grants_only_its_role()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(0);

    let created = app
        .create_user(
            &admin,
            &mut ids,
            &LocalPasswordHasher::lab_v1(),
            NewUserRequest {
                display_name: "Cajera".into(),
                email: "Cajera@HiQNet.Test".into(),
                password: password("cajera"),
                now: utc(NOW),
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(created.user.email().as_str(), "cajera@hiqnet.test");
    assert!(created.roles.is_empty());

    // La contraseña inicial es verificable por el gate comercial del login.
    let login = identity_application(pool.clone())?;
    let mut login_ids = FixtureIds(100);
    let authenticated = login
        .login(
            &mut login_ids,
            &pos_application::identity::LoginRequest {
                email: pos_core::identity::NormalizedEmail::new("cajera@hiqnet.test")?,
                now: utc(NOW),
                occurred_at: at(TIME),
                session_ttl_seconds: 3_600,
            },
            &password("cajera"),
        )
        .await?;
    let handle = SessionHandle::from_encoded(&authenticated.handle)?;
    let hash = handle.hash()?;

    // Sin roles de organización el usuario no obtiene ninguna capacidad.
    let bare = principal(&pool, hash.as_storage_str(), None).await?;
    assert!(bare.permissions().next().is_none());

    let role = app
        .create_role(
            &admin,
            &mut ids,
            "cajero",
            "Cajero",
            &["pos.catalog.read".to_owned()],
            &at(TIME),
        )
        .await?;
    let update = app
        .set_user_roles(
            &admin,
            &mut ids,
            created.user.id().as_str(),
            &[role.role.id().as_str().to_owned()],
            &at(TIME),
        )
        .await?;
    assert_eq!(update.roles.len(), 1);
    assert_eq!(update.roles[0].code().as_str(), "cajero");

    // La autorización se revalida en SQLite: la sesión abierta ya tiene el permiso.
    let granted = principal(&pool, hash.as_storage_str(), None).await?;
    assert!(
        granted
            .permissions()
            .any(|code| code.as_str() == "pos.catalog.read")
    );
    assert!(
        !granted
            .permissions()
            .any(|code| code.as_str() == "user.update")
    );
    Ok(())
}

#[tokio::test]
async fn membership_is_the_only_path_to_branch_permissions()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(200);

    let created = app
        .create_user(
            &admin,
            &mut ids,
            &LocalPasswordHasher::lab_v1(),
            NewUserRequest {
                display_name: "Operador".into(),
                email: "operador@hiqnet.test".into(),
                password: password("operador"),
                now: utc(NOW),
            },
            &at(TIME),
        )
        .await?;
    let role = app
        .create_role(
            &admin,
            &mut ids,
            "cajero-sucursal",
            "Cajero de sucursal",
            &["cash-shift.open".to_owned()],
            &at(TIME),
        )
        .await?;
    // El rol no se asigna como rol de organización: solo debe llegar por la
    // membership, que es exactamente lo que se quiere demostrar.

    let login = identity_application(pool.clone())?;
    let mut login_ids = FixtureIds(300);
    let authenticated = login
        .login(
            &mut login_ids,
            &pos_application::identity::LoginRequest {
                email: pos_core::identity::NormalizedEmail::new("operador@hiqnet.test")?,
                now: utc(NOW),
                occurred_at: at(TIME),
                session_ttl_seconds: 3_600,
            },
            &password("operador"),
        )
        .await?;
    let hash = SessionHandle::from_encoded(&authenticated.handle)?.hash()?;

    // La pierna de organización no aporta nada: el rol solo se asignó por membership.
    let organization_scope = principal(&pool, hash.as_storage_str(), None).await?;
    assert!(organization_scope.permissions().next().is_none());

    // Sin membership, el scope de sucursal tampoco aporta permisos.
    let branch_id = id(BRANCH_A);
    let without = principal(&pool, hash.as_storage_str(), Some(&branch_id)).await?;
    assert!(without.permissions().next().is_none());

    app.create_membership(
        &admin,
        &mut ids,
        BRANCH_A,
        created.user.id().as_str(),
        &[role.role.id().as_str().to_owned()],
        &at(TIME),
    )
    .await?;
    let with_membership = principal(&pool, hash.as_storage_str(), Some(&branch_id)).await?;
    assert!(
        with_membership
            .permissions()
            .any(|code| code.as_str() == "cash-shift.open"),
        "la membership debe habilitar la pierna de sucursal"
    );
    Ok(())
}

#[tokio::test]
async fn self_escalation_and_self_deactivation_are_forbidden()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(400);

    let denied = app
        .set_user_roles(
            &admin,
            &mut ids,
            ADMIN_USER,
            &[ADMIN_ROLE.to_owned()],
            &at(TIME),
        )
        .await;
    assert_eq!(denied, Err(ApplicationError::Forbidden));

    let deactivate = app
        .update_user(
            &admin,
            &mut ids,
            ADMIN_USER,
            UserUpdate {
                expected_revision: 0,
                display_name: None,
                email: None,
                status: Some("INACTIVE".into()),
            },
            &at(TIME),
        )
        .await;
    assert_eq!(deactivate, Err(ApplicationError::Forbidden));
    Ok(())
}

#[tokio::test]
async fn the_organization_never_loses_its_last_administrator()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(500);

    // Quitar `user.update` del único rol que lo concede dejaría la instalación
    // sin administración: se rechaza aunque la revisión sea la vigente.
    let denied = app
        .set_role_permissions(
            &admin,
            &mut ids,
            ADMIN_ROLE,
            "0",
            &["organization.read".to_owned()],
            &at(TIME),
        )
        .await;
    assert_eq!(denied, Err(ApplicationError::LastAdministrator));
    Ok(())
}

#[tokio::test]
async fn stale_revisions_and_duplicates_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(600);

    let created = app
        .create_user(
            &admin,
            &mut ids,
            &LocalPasswordHasher::lab_v1(),
            NewUserRequest {
                display_name: "Repetida".into(),
                email: "repetida@hiqnet.test".into(),
                password: password("repetida"),
                now: utc(NOW),
            },
            &at(TIME),
        )
        .await?;

    let duplicate = app
        .create_user(
            &admin,
            &mut ids,
            &LocalPasswordHasher::lab_v1(),
            NewUserRequest {
                display_name: "Otra".into(),
                email: "repetida@hiqnet.test".into(),
                password: password("repetida-dos"),
                now: utc(NOW),
            },
            &at(TIME),
        )
        .await;
    assert_eq!(duplicate, Err(ApplicationError::UserEmailAlreadyExists));

    let updated = app
        .update_user(
            &admin,
            &mut ids,
            created.user.id().as_str(),
            UserUpdate {
                expected_revision: 0,
                display_name: Some("Repetida II".into()),
                email: None,
                status: None,
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(updated.user.revision(), 1);

    let stale = app
        .update_user(
            &admin,
            &mut ids,
            created.user.id().as_str(),
            UserUpdate {
                expected_revision: 0,
                display_name: Some("Repetida III".into()),
                email: None,
                status: None,
            },
            &at(LATER),
        )
        .await;
    assert_eq!(stale, Err(ApplicationError::ConcurrentModification));

    let role = app
        .create_role(&admin, &mut ids, "cajero", "Cajero", &[], &at(TIME))
        .await?;
    let duplicate_role = app
        .create_role(&admin, &mut ids, "cajero", "Cajero II", &[], &at(TIME))
        .await;
    assert_eq!(duplicate_role, Err(ApplicationError::RoleCodeAlreadyExists));

    let renamed = app
        .update_role(
            &admin,
            &mut ids,
            role.role.id().as_str(),
            RoleUpdate {
                expected_revision: 0,
                name: Some("Cajero nocturno".into()),
                status: None,
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(renamed.revision(), 1);
    Ok(())
}

#[tokio::test]
async fn foreign_organizations_are_not_visible() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(700);

    let foreign = app.get_user(&admin, FOREIGN_USER).await;
    assert_eq!(foreign, Err(ApplicationError::UserNotFound));

    let foreign_role = app.get_role(&admin, FOREIGN_ROLE).await;
    assert_eq!(foreign_role, Err(ApplicationError::RoleNotFound));

    let foreign_membership = app
        .create_membership(&admin, &mut ids, BRANCH_A, FOREIGN_USER, &[], &at(TIME))
        .await;
    assert_eq!(foreign_membership, Err(ApplicationError::UserNotFound));
    Ok(())
}

#[tokio::test]
async fn deactivation_revokes_sessions_and_persists_across_reopen()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(800);

    let created = app
        .create_user(
            &admin,
            &mut ids,
            &LocalPasswordHasher::lab_v1(),
            NewUserRequest {
                display_name: "Temporal".into(),
                email: "temporal@hiqnet.test".into(),
                password: password("temporal"),
                now: utc(NOW),
            },
            &at(TIME),
        )
        .await?;
    let login = identity_application(pool.clone())?;
    let mut login_ids = FixtureIds(900);
    let authenticated = login
        .login(
            &mut login_ids,
            &pos_application::identity::LoginRequest {
                email: pos_core::identity::NormalizedEmail::new("temporal@hiqnet.test")?,
                now: utc(NOW),
                occurred_at: at(TIME),
                session_ttl_seconds: 3_600,
            },
            &password("temporal"),
        )
        .await?;
    let hash = SessionHandle::from_encoded(&authenticated.handle)?.hash()?;

    app.update_user(
        &admin,
        &mut ids,
        created.user.id().as_str(),
        UserUpdate {
            expected_revision: 0,
            display_name: None,
            email: None,
            status: Some("INACTIVE".into()),
        },
        &at(LATER),
    )
    .await?;

    // La sesión de la cuenta desactivada ya no autentica.
    let denied = principal(&pool, hash.as_storage_str(), None).await;
    assert!(denied.is_err());

    pool.close().await;
    let reopened = open(
        &temp.path().join("identity.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    let users = application(&reopened).list_users(&admin, 50, 0).await?;
    assert!(users.iter().any(|summary| {
        summary.user.email().as_str() == "temporal@hiqnet.test"
            && summary.user.status() == pos_core::identity::PosUserStatus::Inactive
    }));
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn membership_roles_are_replaced_with_cas() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(1_000);

    let created = app
        .create_user(
            &admin,
            &mut ids,
            &LocalPasswordHasher::lab_v1(),
            NewUserRequest {
                display_name: "Operador".into(),
                email: "membresia@hiqnet.test".into(),
                password: password("membresia"),
                now: utc(NOW),
            },
            &at(TIME),
        )
        .await?;
    let role = app
        .create_role(&admin, &mut ids, "cajero", "Cajero", &[], &at(TIME))
        .await?;
    let membership = app
        .create_membership(
            &admin,
            &mut ids,
            BRANCH_A,
            created.user.id().as_str(),
            &[],
            &at(TIME),
        )
        .await?;
    assert_eq!(membership.membership.revision(), 0);

    let updated = app
        .update_membership(
            &admin,
            &mut ids,
            membership.membership.id().as_str(),
            MembershipUpdate {
                expected_revision: 0,
                status: None,
                role_ids: Some(vec![role.role.id().as_str().to_owned()]),
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(updated.membership.revision(), 1);
    assert_eq!(updated.roles.len(), 1);

    let stale = app
        .update_membership(
            &admin,
            &mut ids,
            membership.membership.id().as_str(),
            MembershipUpdate {
                expected_revision: 0,
                status: Some("INACTIVE".into()),
                role_ids: None,
            },
            &at(LATER),
        )
        .await;
    assert_eq!(stale, Err(ApplicationError::ConcurrentModification));

    let duplicate = app
        .create_membership(
            &admin,
            &mut ids,
            BRANCH_A,
            created.user.id().as_str(),
            &[],
            &at(TIME),
        )
        .await;
    assert_eq!(duplicate, Err(ApplicationError::MembershipAlreadyExists));
    Ok(())
}

#[tokio::test]
async fn permissions_catalog_is_readable_and_codes_are_validated()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = candidate().await?;
    let admin = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut ids = FixtureIds(1_100);

    let catalog = app.list_permissions(&admin).await?;
    assert!(catalog.iter().any(|(code, _)| code == "user.update"));

    // El administrador siembra `permission.read` indirectamente: se comprueba con
    // el rol de sistema, que no puede inventar códigos.
    let invalid = app
        .create_role(
            &admin,
            &mut ids,
            "inventado",
            "Inventado",
            &["permiso.inexistente".to_owned()],
            &at(TIME),
        )
        .await;
    assert_eq!(invalid, Err(ApplicationError::InvalidAdministrationInput));

    let password_only = app
        .create_role(
            &admin,
            &mut ids,
            "sin-permisos",
            "Sin permisos",
            &[String::new()],
            &at(TIME),
        )
        .await;
    assert_eq!(
        password_only,
        Err(ApplicationError::InvalidAdministrationInput)
    );
    Ok(())
}
