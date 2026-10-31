//! Catálogo persistente sobre el esquema **candidato** y SQLite real.
//!
//! Las tablas vienen de `proposals/local-catalog/0008_catalog.proposed.sql`; la
//! auditoría y el principal verificado, de `proposals/local-auth/`. Nada de esto
//! está cargado por `pos-local`: la prueba demuestra que la composición
//! (autorización DB-authoritative + UoW + CAS + auditoría) es correcta para que,
//! una vez aprobado el DDL, la promoción no requiera reescribir lógica.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_core::{
    audit::AuditOccurredAt,
    catalog::PatchField,
    identity::{
        LocalPrincipalService, PermissionCode, PrincipalScope, SessionHash, UtcSeconds,
        VerifiedPrincipal,
    },
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        IsoCurrency, LocaleTag, NewBranch, NewOrganization, Organization, Status, Timezone,
    },
};
use sqlx::SqlitePool;

use super::*;
use crate::{
    catalog::{
        CatalogPage, CategoryUpdateRequest, MAX_PAGE_SIZE, NewCategoryRequest, NewProductRequest,
        ProductUpdateRequest, SqliteCatalogApplication,
    },
    identity::SqliteIdentityLab,
};

const PROPOSALS: [&str; 7] = [
    include_str!("../../../proposals/local-auth/0002_installation_identity.proposed.sql"),
    include_str!("../../../proposals/local-auth/0003_user_credential_session.proposed.sql"),
    include_str!("../../../proposals/local-auth/0004_rbac_catalog.proposed.sql"),
    include_str!("../../../proposals/local-auth/0005_branch_membership.proposed.sql"),
    include_str!("../../../proposals/local-auth/0006_audit_event.proposed.sql"),
    include_str!("../../../proposals/local-auth/0007_bootstrap_record.proposed.sql"),
    include_str!("../../../proposals/local-catalog/0008_catalog.proposed.sql"),
];

const ORG_A: &str = "00000000-0000-0000-0000-000000000080";
const ORG_B: &str = "00000000-0000-0000-0000-000000000081";
const USER_A: &str = "00000000-0000-0000-0000-0000000000a0";
const USER_B: &str = "00000000-0000-0000-0000-0000000000a1";
const ROLE_A: &str = "00000000-0000-0000-0000-0000000000b0";
const ROLE_B: &str = "00000000-0000-0000-0000-0000000000b1";
const SESSION_A: &str = "00000000-0000-0000-0000-0000000000d0";
const SESSION_B: &str = "00000000-0000-0000-0000-0000000000d1";
const BRANCH_A: &str = "00000000-0000-0000-0000-000000000090";
const MEMBERSHIP_A: &str = "00000000-0000-0000-0000-0000000000c0";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const HASH_A: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";
const HASH_B: &str = "bbbbbbbbccccccccddddddddeeeeeeeeffffffff000000001111111122222222";
/// Instante de autenticación dentro de la ventana de sesión.
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
            name: DisplayName::new("Catálogo").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    )
}

/// Generador determinista que nunca repite identidades.
struct FixtureIds(u32);

impl IdentityGenerator for FixtureIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        self.0 += 1;
        EntityId::parse(&format!("00000000-0000-0000-0000-{:012x}", 0x100 + self.0))
            .map_err(|_| ApplicationError::Storage)
    }
}

async fn open_candidate() -> Result<(tempfile::TempDir, SqlitePool), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("catalog.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    for statement in PROPOSALS {
        sqlx::raw_sql(statement).execute(&pool).await?;
    }
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    store
        .insert_organization(&organization(ORG_A, "catalog-a"), TIME, TIME)
        .await?;
    store
        .insert_organization(&organization(ORG_B, "catalog-b"), TIME, TIME)
        .await?;
    store
        .insert_branch(
            &Branch::create(
                id(BRANCH_A),
                id(ORG_A),
                NewBranch {
                    name: DisplayName::new("Sucursal")
                        .unwrap_or_else(|error| panic!("name: {error}")),
                    code: BusinessCode::new("centro")
                        .unwrap_or_else(|error| panic!("code: {error}")),
                    timezone_override: None,
                },
            ),
            TIME,
            TIME,
        )
        .await?;
    seed_security(&pool).await?;
    Ok((temp, pool))
}

async fn seed_security(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for (user, owner, email) in [
        (USER_A, ORG_A, "catalogo-a@hiqnet.test"),
        (USER_B, ORG_B, "catalogo-b@hiqnet.test"),
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
    for (role, owner, code) in [
        (ROLE_A, ORG_A, "catalog-admin"),
        (ROLE_B, ORG_B, "catalog-reader"),
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
    // A recibe el catálogo completo; B solo lectura, para probar deny-by-default.
    for permission in [
        "category.create",
        "category.read",
        "category.update",
        "product.create",
        "product.read",
        "product.update",
        "pos.catalog.read",
    ] {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(ROLE_A)
            .bind(permission)
            .execute(pool)
            .await?;
    }
    sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
        .bind(ROLE_B)
        .bind("product.read")
        .execute(pool)
        .await?;
    // B puede leer el catálogo y crear productos, pero no administra categorías
    // ni modifica productos: así se prueba deny-by-default sin fingir un permiso.
    for permission in ["product.create", "category.read"] {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(ROLE_B)
            .bind(permission)
            .execute(pool)
            .await?;
    }
    for (owner, user, role) in [(ORG_A, USER_A, ROLE_A), (ORG_B, USER_B, ROLE_B)] {
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
    // Membership de sucursal: prueba que un principal con scope de sucursal no
    // autoriza el catálogo organizacional aunque su rol tenga los permisos.
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
    .bind(ROLE_A)
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

fn application(pool: &SqlitePool) -> SqliteCatalogApplication {
    SqliteCatalogApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

async fn create_category(
    app: &SqliteCatalogApplication,
    owner: &VerifiedPrincipal,
    code: &str,
    name: &str,
) -> Result<pos_core::catalog::Category, ApplicationError> {
    let mut identities = FixtureIds(0);
    app.create_category(
        owner,
        &mut identities,
        NewCategoryRequest {
            code: code.to_owned(),
            name: name.to_owned(),
        },
        &at(TIME),
    )
    .await
}

fn new_product(sku: &str, price: &str, category_id: Option<EntityId>) -> NewProductRequest {
    NewProductRequest {
        sku: sku.to_owned(),
        name: "Producto".to_owned(),
        unit_price: price.to_owned(),
        barcode: None,
        description: None,
        category_id,
    }
}

#[tokio::test]
async fn category_and_product_persist_with_audit_and_survive_reopen()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);

    // El código y el nombre llegan sin normalizar, como en un transporte HTTP.
    let category = create_category(&app, &owner, "  Bebidas ", "  Bebidas Frías  ").await?;
    assert_eq!(category.code().as_str(), "bebidas");
    assert_eq!(category.name().as_str(), "Bebidas Frías");
    assert_eq!(category.status(), Status::Active);
    assert_eq!(category.revision(), 0);

    let mut identities = FixtureIds(1);
    let product = app
        .create_product(
            &owner,
            &mut identities,
            NewProductRequest {
                sku: " cafe-01 ".to_owned(),
                name: "Café de olla".to_owned(),
                unit_price: "12.5".to_owned(),
                barcode: Some("000123".to_owned()),
                description: Some("   ".to_owned()),
                category_id: Some(category.id().clone()),
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(product.sku().as_str(), "CAFE-01");
    assert_eq!(product.unit_price().to_string(), "12.5000");
    assert_eq!(
        product.barcode().map(pos_core::catalog::Barcode::as_str),
        Some("000123"),
        "los ceros iniciales se conservan"
    );
    assert!(
        product.description().is_none(),
        "blanco se normaliza a None"
    );
    assert_eq!(product.category_id(), Some(category.id()));

    let stored = app.get_product(&owner, product.id()).await?;
    assert_eq!(stored, product);

    let listed = app
        .list_products(
            &owner,
            None,
            None,
            CatalogPage {
                limit: 10,
                offset: 0,
            },
        )
        .await?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed.first().map(|item| item.sku().as_str()),
        Some("CAFE-01")
    );

    // Auditoría: dos hechos USER con la sesión del principal.
    let events: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT action, actor_type, actor_user_id, session_id FROM audit_event ORDER BY action",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(events.len(), 2);
    assert_eq!(
        events.first().map(|row| row.0.as_str()),
        Some("category.created")
    );
    assert_eq!(
        events.get(1).map(|row| row.0.as_str()),
        Some("product.created")
    );
    for (_, actor_type, actor, session) in &events {
        assert_eq!(actor_type, "USER");
        assert_eq!(actor.as_str(), USER_A);
        assert_eq!(session.as_deref(), Some(SESSION_A));
    }

    pool.close().await;
    let reopened = open(
        &temp.path().join("catalog.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    let durable: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product")
        .fetch_one(&reopened)
        .await?;
    assert_eq!(durable, 1, "el catálogo sobrevive a la reapertura");
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn product_price_update_uses_cas_and_skips_noop_audit()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(10);
    let product = app
        .create_product(
            &owner,
            &mut identities,
            new_product("SKU-1", "10.00", None),
            &at(TIME),
        )
        .await?;

    let request = |version: u64, price: &str| ProductUpdateRequest {
        expected_version: version,
        name: None,
        description: PatchField::Absent,
        barcode: PatchField::Absent,
        unit_price: Some(price.to_owned()),
        category_id: PatchField::Absent,
        status: None,
    };

    let updated = app
        .update_product(
            &owner,
            &mut identities,
            product.id(),
            request(0, "15.75"),
            &at(LATER),
        )
        .await?;
    assert_eq!(updated.unit_price().to_string(), "15.7500");
    assert_eq!(updated.revision(), 1);

    // Versión obsoleta: conflicto y sin efectos.
    assert_eq!(
        app.update_product(
            &owner,
            &mut identities,
            product.id(),
            request(0, "99.00"),
            &at(LATER)
        )
        .await,
        Err(ApplicationError::ConcurrentModification)
    );
    let unchanged = app.get_product(&owner, product.id()).await?;
    assert_eq!(unchanged.unit_price().to_string(), "15.7500");

    // No-op: no incrementa versión ni escribe auditoría.
    let same = app
        .update_product(
            &owner,
            &mut identities,
            product.id(),
            request(1, "15.75"),
            &at(LATER),
        )
        .await?;
    assert_eq!(same.revision(), 1);

    let updated_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_event WHERE action = 'product.updated'")
            .fetch_one(&pool)
            .await?;
    assert_eq!(updated_events, 1, "una actualización material, un hecho");

    let metadata: String = sqlx::query_scalar(
        "SELECT metadata_json FROM audit_event WHERE action = 'product.updated'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(metadata, r#"{"changedFields":["unitPrice"]}"#);
    Ok(())
}

#[tokio::test]
async fn category_update_deactivates_and_blocks_product_assignment()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    let category = create_category(&app, &owner, "postres", "Postres").await?;

    let mut identities = FixtureIds(20);
    let inactive = app
        .update_category(
            &owner,
            &mut identities,
            category.id(),
            CategoryUpdateRequest {
                expected_version: 0,
                name: None,
                status: Some(Status::Inactive),
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(inactive.status(), Status::Inactive);
    assert_eq!(inactive.revision(), 1);

    assert_eq!(
        app.update_category(
            &owner,
            &mut identities,
            category.id(),
            CategoryUpdateRequest {
                expected_version: 0,
                name: None,
                status: Some(Status::Active),
            },
            &at(LATER),
        )
        .await,
        Err(ApplicationError::ConcurrentModification)
    );

    assert_eq!(
        app.create_product(
            &owner,
            &mut identities,
            new_product("SKU-POSTRE", "5.00", Some(category.id().clone())),
            &at(LATER),
        )
        .await,
        Err(ApplicationError::CategoryInactive)
    );
    let metadata: String = sqlx::query_scalar(
        "SELECT metadata_json FROM audit_event WHERE action = 'category.updated'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(metadata, r#"{"changedFields":["status"]}"#);
    Ok(())
}

#[tokio::test]
async fn duplicate_codes_skus_and_barcodes_are_scoped_per_organization()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner_a = principal(&pool, HASH_A, None).await?;
    let owner_b = principal(&pool, HASH_B, None).await?;
    let app = application(&pool);

    create_category(&app, &owner_a, "bebidas", "Bebidas").await?;
    assert_eq!(
        create_category(&app, &owner_a, "BEBIDAS", "Otra").await,
        Err(ApplicationError::CategoryCodeAlreadyExists)
    );

    let mut identities = FixtureIds(30);
    app.create_product(
        &owner_a,
        &mut identities,
        NewProductRequest {
            barcode: Some("750100".to_owned()),
            ..new_product("SKU-DUP", "1.00", None)
        },
        &at(TIME),
    )
    .await?;
    assert_eq!(
        app.create_product(
            &owner_a,
            &mut identities,
            new_product("sku-dup", "2.00", None),
            &at(TIME)
        )
        .await,
        Err(ApplicationError::ProductSkuAlreadyExists)
    );
    assert_eq!(
        app.create_product(
            &owner_a,
            &mut identities,
            NewProductRequest {
                barcode: Some("750100".to_owned()),
                ..new_product("SKU-OTRO", "2.00", None)
            },
            &at(TIME),
        )
        .await,
        Err(ApplicationError::ProductBarcodeAlreadyExists)
    );

    // La misma identidad de negocio en otra organización sí es válida.
    let mut identities_b = FixtureIds(40);
    app.create_product(
        &owner_b,
        &mut identities_b,
        new_product("SKU-DUP", "3.00", None),
        &at(TIME),
    )
    .await?;
    let mirrored = app
        .list_products(
            &owner_b,
            None,
            None,
            CatalogPage {
                limit: 10,
                offset: 0,
            },
        )
        .await?;
    assert_eq!(mirrored.len(), 1);
    Ok(())
}

#[tokio::test]
async fn category_of_another_organization_is_invisible_and_blocked_by_the_schema()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner_a = principal(&pool, HASH_A, None).await?;
    let owner_b = principal(&pool, HASH_B, None).await?;
    let app = application(&pool);
    let foreign = create_category(&app, &owner_a, "privada", "Privada").await?;

    // Lectura cross-org: se comporta como inexistente, sin revelar el recurso.
    assert_eq!(
        app.get_category(&owner_b, foreign.id()).await,
        Err(ApplicationError::CategoryNotFound)
    );

    // B no administra categorías: el permiso se deniega antes de tocar SQL.
    let mut identities = FixtureIds(50);
    assert_eq!(
        app.create_category(
            &owner_b,
            &mut identities,
            NewCategoryRequest {
                code: "ajena".to_owned(),
                name: "Ajena".to_owned(),
            },
            &at(TIME)
        )
        .await,
        Err(ApplicationError::Forbidden)
    );

    // Barrera física: incluso una escritura SQL directa con la categoría ajena falla.
    let result = sqlx::query(
        "INSERT INTO product (id, organization_id, category_id, sku, barcode, name, description, \
         unit_price, status, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000000ff', ?, ?, 'RAW-1', NULL, 'Crudo', NULL, \
         '1.0000', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG_B)
    .bind(foreign.id().as_str())
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "la FK compuesta same-org debe rechazar la fila"
    );
    Ok(())
}

#[tokio::test]
async fn audit_failure_rolls_back_the_catalog_write() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);

    // Sin tabla de auditoría la operación no puede completarse: la mutación no
    // debe quedar confirmada.
    sqlx::query("DROP TABLE audit_event").execute(&pool).await?;
    let result = create_category(&app, &owner, "sin-auditoria", "Sin auditoría").await;
    assert!(
        result.is_err(),
        "una auditoría obligatoria fallida aborta el caso de uso"
    );

    let persisted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM category")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        persisted, 0,
        "no puede quedar una categoría sin su hecho auditable"
    );
    Ok(())
}

#[tokio::test]
async fn authorization_is_deny_by_default_and_requires_organization_scope()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    create_category(&app, &owner, "bebidas", "Bebidas").await?;

    // El principal de B solo puede leer productos.
    let reader = principal(&pool, HASH_B, None).await?;
    let mut identities = FixtureIds(60);
    assert_eq!(
        app.create_category(
            &reader,
            &mut identities,
            NewCategoryRequest {
                code: "postres".to_owned(),
                name: "Postres".to_owned(),
            },
            &at(TIME),
        )
        .await,
        Err(ApplicationError::Forbidden)
    );
    assert!(
        app.list_products(
            &reader,
            None,
            None,
            CatalogPage {
                limit: 10,
                offset: 0
            }
        )
        .await
        .is_ok()
    );

    // Un principal resuelto para una sucursal no autoriza el catálogo
    // organizacional aunque su rol tenga category.create.
    let branch_principal = principal(&pool, HASH_A, Some(&id(BRANCH_A))).await?;
    assert_eq!(branch_principal.scope().branch_id(), Some(&id(BRANCH_A)));
    assert_eq!(
        app.create_category(
            &branch_principal,
            &mut identities,
            NewCategoryRequest {
                code: "sucursal".to_owned(),
                name: "Sucursal".to_owned(),
            },
            &at(TIME),
        )
        .await,
        Err(ApplicationError::Forbidden),
        "el scope de sucursal no puede autorizar una operación de organización"
    );

    // El mismo usuario, resuelto en alcance organizacional, sí queda autorizado.
    let scope = PrincipalScope::new(id(ORG_A), None);
    assert_eq!(owner.scope(), &scope);
    assert!(
        owner
            .authorize(
                &PermissionCode::new("category.create")
                    .unwrap_or_else(|error| panic!("permiso: {error}")),
                &scope
            )
            .is_ok()
    );
    Ok(())
}

#[tokio::test]
async fn page_bounds_are_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, None).await?;
    let app = application(&pool);
    assert_eq!(
        app.list_products(
            &owner,
            None,
            None,
            CatalogPage {
                limit: 0,
                offset: 0
            }
        )
        .await,
        Err(ApplicationError::InvalidCatalogInput)
    );
    assert_eq!(
        app.list_categories(
            &owner,
            None,
            CatalogPage {
                limit: MAX_PAGE_SIZE + 1,
                offset: 0
            }
        )
        .await,
        Err(ApplicationError::InvalidCatalogInput)
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_price_updates_commit_only_one_revision()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, first_pool) = open_candidate().await?;
    let owner = principal(&first_pool, HASH_A, None).await?;
    let app = application(&first_pool);
    let mut identities = FixtureIds(70);
    let product = app
        .create_product(
            &owner,
            &mut identities,
            new_product("SKU-CAS", "1.00", None),
            &at(TIME),
        )
        .await?;

    // Segundo pool independiente sobre el mismo archivo, como dos clientes.
    let second_pool = open(
        &temp.path().join("catalog.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    let second_app = application(&second_pool);

    let request = ProductUpdateRequest {
        expected_version: 0,
        name: None,
        description: PatchField::Absent,
        barcode: PatchField::Absent,
        unit_price: Some("2.00".to_owned()),
        category_id: PatchField::Absent,
        status: None,
    };
    let mut first_ids = FixtureIds(80);
    let mut second_ids = FixtureIds(90);
    assert!(
        app.update_product(
            &owner,
            &mut first_ids,
            product.id(),
            request.clone(),
            &at(LATER)
        )
        .await
        .is_ok()
    );
    assert_eq!(
        second_app
            .update_product(&owner, &mut second_ids, product.id(), request, &at(LATER))
            .await,
        Err(ApplicationError::ConcurrentModification),
        "solo un escritor puede confirmar la revisión 0"
    );
    let final_product = app.get_product(&owner, product.id()).await?;
    assert_eq!(final_product.revision(), 1);
    assert_eq!(final_product.unit_price().to_string(), "2.0000");
    second_pool.close().await;
    Ok(())
}
