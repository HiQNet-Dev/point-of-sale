//! Integración SQLite real de compras sobre el esquema candidato `0016`.
//!
//! El fixture aplica explícitamente las propuestas `0002`–`0016`; el migrador
//! productivo no las descubre. Cada prueba usa SQLite real, auditoría real y la
//! misma `SqliteUnitOfWork` que usará el producto tras la aprobación.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_core::{
    audit::AuditOccurredAt,
    identity::{LocalPrincipalService, SessionHash, UtcSeconds, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        IsoCurrency, LocaleTag, NewBranch, NewOrganization, Organization, Timezone,
    },
};
use sqlx::SqlitePool;

use super::*;
use crate::{
    identity::SqliteIdentityLab,
    purchasing::{
        ChangeSupplierRequest, NewPurchaseOrderLineRequest, NewPurchaseOrderRequest,
        NewSupplierRequest, PostReceiptRequest, PurchasingPage, ReceiptLineInput,
        SqlitePurchasingApplication, UpdatePurchaseOrderLineRequest, UpdateSupplierRequest,
    },
};

const ORG_A: &str = "00000000-0000-0000-0000-000000000380";
const ORG_B: &str = "00000000-0000-0000-0000-000000000381";
const BRANCH_A: &str = "00000000-0000-0000-0000-000000000390";
const BRANCH_OTHER: &str = "00000000-0000-0000-0000-000000000392";
const USER_A: &str = "00000000-0000-0000-0000-0000000003a0";
const USER_B: &str = "00000000-0000-0000-0000-0000000003a1";
const ROLE_A: &str = "00000000-0000-0000-0000-0000000003b0";
const SESSION_A: &str = "00000000-0000-0000-0000-0000000003d0";
const SESSION_B: &str = "00000000-0000-0000-0000-0000000003d1";
const HASH_A: &str = "3333333344444444555555556666666677777777888888889999999900000000";
const HASH_B: &str = "4444444455555555666666667777777788888888999999990000000011111111";
const MEMBERSHIP_A: &str = "00000000-0000-0000-0000-0000000003c0";
const ITEM_A: &str = "00000000-0000-0000-0000-0000000003e0";
const ITEM_G: &str = "00000000-0000-0000-0000-0000000003e1";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const NOW: i64 = 1_793_235_600;

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn at(value: &str) -> AuditOccurredAt {
    AuditOccurredAt::new(value).unwrap_or_else(|error| panic!("instante: {error}"))
}

struct FixtureIds(u32);
impl IdentityGenerator for FixtureIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        self.0 += 1;
        EntityId::parse(&format!("00000000-0000-0000-0000-{:012x}", 0x400 + self.0))
            .map_err(|_| ApplicationError::Storage)
    }
}

fn application(pool: &SqlitePool) -> SqlitePurchasingApplication {
    SqlitePurchasingApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

fn new_supplier(code: &str) -> NewSupplierRequest {
    NewSupplierRequest {
        code: code.to_owned(),
        name: "Distribuidora Centro".to_owned(),
        legal_name: Some("Distribuidora Centro S.A.".to_owned()),
        tax_id: Some("RFC-123".to_owned()),
        contact_name: Some("Ana".to_owned()),
        email: Some("ANA@Example.COM".to_owned()),
        phone: Some("555-0101".to_owned()),
        address: Some("Calle 1".to_owned()),
    }
}

async fn open_candidate() -> Result<(tempfile::TempDir, SqlitePool), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("purchasing.sqlite"),
        Duration::from_millis(500),
    )
    .await?;
    migrate(&pool).await?;

    let store = SqliteOrganizationBranchStore::new(pool.clone());
    for (organization_id, code) in [(ORG_A, "operacion-a"), (ORG_B, "operacion-b")] {
        store
            .insert_organization(
                &Organization::create(
                    id(organization_id),
                    NewOrganization {
                        name: DisplayName::new("Operación")
                            .unwrap_or_else(|error| panic!("name: {error}")),
                        code: BusinessCode::new(code)
                            .unwrap_or_else(|error| panic!("code: {error}")),
                        timezone: Timezone::new("UTC")
                            .unwrap_or_else(|error| panic!("tz: {error}")),
                        currency: IsoCurrency::new("MXN")
                            .unwrap_or_else(|error| panic!("currency: {error}")),
                        locale: LocaleTag::new("es-MX")
                            .unwrap_or_else(|error| panic!("locale: {error}")),
                    },
                ),
                TIME,
                TIME,
            )
            .await?;
    }
    for (branch_id, organization_id, code) in [
        (BRANCH_A, ORG_A, "centro"),
        (BRANCH_OTHER, ORG_B, "externa"),
    ] {
        store
            .insert_branch(
                &Branch::create(
                    id(branch_id),
                    id(organization_id),
                    NewBranch {
                        name: DisplayName::new("Sucursal")
                            .unwrap_or_else(|error| panic!("name: {error}")),
                        code: BusinessCode::new(code)
                            .unwrap_or_else(|error| panic!("code: {error}")),
                        timezone_override: None,
                    },
                ),
                TIME,
                TIME,
            )
            .await?;
    }
    seed_security(&pool).await?;
    seed_inventory(&pool).await?;
    Ok((temp, pool))
}

async fn seed_security(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for (user, organization_id, email) in [
        (USER_A, ORG_A, "compras-a@hiqnet.test"),
        (USER_B, ORG_B, "compras-b@hiqnet.test"),
    ] {
        sqlx::query(
            "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES (?, ?, 'Compradora', ?, 'ACTIVE', 0, ?, ?)",
        )
        .bind(user)
        .bind(organization_id)
        .bind(email)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
         VALUES (?, ?, 'purchasing-admin', 'Compras', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ROLE_A)
    .bind(ORG_A)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    for permission in [
        "purchasing.supplier.read",
        "purchasing.supplier.write",
        "purchasing.order.read",
        "purchasing.order.write",
        "purchasing.order.issue",
        "purchasing.order.cancel",
        "purchasing.receipt.read",
        "purchasing.receipt.write",
        "inventory.stock.read",
    ] {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(ROLE_A)
            .bind(permission)
            .execute(pool)
            .await?;
    }
    sqlx::query(
        "INSERT INTO user_organization_role (organization_id, user_id, role_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(USER_A)
    .bind(ROLE_A)
    .bind(TIME)
    .execute(pool)
    .await?;
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
        "INSERT INTO branch_membership_role (organization_id, membership_id, role_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(ORG_A)
    .bind(MEMBERSHIP_A)
    .bind(ROLE_A)
    .bind(TIME)
    .execute(pool)
    .await?;
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
    Ok(())
}

async fn seed_inventory(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for (item, code, name, unit, fractional) in [
        (ITEM_A, "harina", "Harina", "UNIT", 0),
        (ITEM_G, "azucar", "Azúcar", "GRAM", 1),
    ] {
        sqlx::query(
            "INSERT INTO inventory_item (id, organization_id, code, name, description, base_unit, \
             fractional_unit_allowed, unit_locked, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, ?, NULL, ?, ?, 0, 'ACTIVE', 0, ?, ?)",
        )
        .bind(item)
        .bind(ORG_A)
        .bind(code)
        .bind(name)
        .bind(unit)
        .bind(fractional)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn principal(
    pool: &SqlitePool,
    hash: &str,
    branch_id: &EntityId,
) -> Result<VerifiedPrincipal, Box<dyn std::error::Error>> {
    let mut connection = pool.acquire().await?;
    let hash = SessionHash::new(hash)?;
    let result = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection))
        .authenticate(&hash, UtcSeconds::new(NOW)?, Some(branch_id))
        .await;
    connection.close().await?;
    Ok(result?)
}

async fn seeded_supplier(
    app: &SqlitePurchasingApplication,
    principal: &VerifiedPrincipal,
    identities: &mut FixtureIds,
) -> String {
    app.create_supplier(
        principal,
        identities,
        new_supplier("proveedor-1"),
        &at(TIME),
    )
    .await
    .unwrap_or_else(|error| panic!("supplier: {error}"))
    .id
}

async fn draft_order(
    app: &SqlitePurchasingApplication,
    principal: &VerifiedPrincipal,
    identities: &mut FixtureIds,
    supplier_id: &str,
    code: &str,
) -> pos_application::purchasing::PurchaseOrderView {
    app.create_purchase_order(
        principal,
        identities,
        NewPurchaseOrderRequest {
            code: code.to_owned(),
            supplier_id: id(supplier_id),
            lines: vec![NewPurchaseOrderLineRequest {
                inventory_item_id: id(ITEM_A),
                quantity: "10.000000".to_owned(),
                unit_cost: Some("2.5000".to_owned()),
            }],
        },
        &at(TIME),
    )
    .await
    .unwrap_or_else(|error| panic!("order: {error}"))
}

#[tokio::test]
async fn supplier_lifecycle_persists_and_survives_reopen() -> Result<(), Box<dyn std::error::Error>>
{
    let (temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);

    let created = app
        .create_supplier(&user, &mut ids, new_supplier("proveedor-1"), &at(TIME))
        .await?;
    assert_eq!(created.code, "proveedor-1");
    assert_eq!(created.email.as_deref(), Some("ana@example.com"));
    assert_eq!(created.version, "0");

    let listed = app
        .list_suppliers(
            &user,
            None,
            PurchasingPage {
                limit: 20,
                offset: 0,
            },
        )
        .await?;
    assert_eq!(listed.len(), 1);

    let updated = app
        .update_supplier(
            &user,
            &mut ids,
            &id(&created.id),
            UpdateSupplierRequest {
                expected_version: 0,
                name: Some("Distribuidora Norte".to_owned()),
                legal_name: pos_application::purchasing::OptionalField::Absent,
                tax_id: pos_application::purchasing::OptionalField::Absent,
                contact_name: pos_application::purchasing::OptionalField::Absent,
                email: pos_application::purchasing::OptionalField::Absent,
                phone: pos_application::purchasing::OptionalField::Absent,
                address: pos_application::purchasing::OptionalField::Clear,
                status: None,
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(updated.name, "Distribuidora Norte");
    assert_eq!(updated.version, "1");

    let audits: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE action = 'supplier.created'), \
         COUNT(*) FILTER (WHERE action = 'supplier.updated') FROM audit_event",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(audits, (1, 1));
    pool.close().await;

    let reopened = open(
        &temp.path().join("purchasing.sqlite"),
        Duration::from_millis(500),
    )
    .await?;
    let name: String = sqlx::query_scalar("SELECT name FROM supplier WHERE id = ?")
        .bind(&created.id)
        .fetch_one(&reopened)
        .await?;
    assert_eq!(name, "Distribuidora Norte");
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn duplicate_slug_and_cross_organization_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    app.create_supplier(&user, &mut ids, new_supplier("proveedor-1"), &at(TIME))
        .await?;
    assert_eq!(
        app.create_supplier(&user, &mut ids, new_supplier("proveedor-1"), &at(TIME))
            .await
            .err(),
        Some(ApplicationError::SupplierCodeAlreadyExists)
    );

    // Un principal de otra organización no ve el proveedor: el scope se aplica
    // antes de cualquier búsqueda y responde como inexistente.
    let foreign = principal(&pool, HASH_B, &id(BRANCH_OTHER)).await?;
    assert_eq!(
        app.get_supplier(&foreign, &id("00000000-0000-0000-0000-0000000004ff"))
            .await
            .err(),
        Some(ApplicationError::Forbidden)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn order_snapshots_and_unknown_cost_stay_distinct_from_zero()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;

    let order = app
        .create_purchase_order(
            &user,
            &mut ids,
            NewPurchaseOrderRequest {
                code: "oc-1".to_owned(),
                supplier_id: id(&supplier),
                lines: vec![
                    NewPurchaseOrderLineRequest {
                        inventory_item_id: id(ITEM_A),
                        quantity: "2.000000".to_owned(),
                        unit_cost: Some("1.5000".to_owned()),
                    },
                    NewPurchaseOrderLineRequest {
                        inventory_item_id: id(ITEM_G),
                        quantity: "500.000000".to_owned(),
                        unit_cost: None,
                    },
                ],
            },
            &at(TIME),
        )
        .await?;

    assert_eq!(order.status, "DRAFT");
    assert_eq!(order.supplier_code, "proveedor-1");
    assert_eq!(order.currency, "MXN");
    assert_eq!(order.lines.len(), 2);
    assert_eq!(order.lines[0].unit_cost.as_deref(), Some("1.5000"));
    assert_eq!(order.lines[0].line_total.as_deref(), Some("3.0000000000"));
    assert_eq!(order.lines[1].unit_cost, None);
    assert_eq!(order.lines[1].line_total, None);
    // El total es desconocido: nunca una suma parcial disfrazada.
    assert_eq!(order.total, None);

    let stored: Option<String> =
        sqlx::query_scalar("SELECT unit_cost FROM purchase_order_line WHERE unit_cost IS NULL")
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored, None);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn zero_cost_is_persisted_as_zero_not_null() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = app
        .create_purchase_order(
            &user,
            &mut ids,
            NewPurchaseOrderRequest {
                code: "oc-zero".to_owned(),
                supplier_id: id(&supplier),
                lines: vec![NewPurchaseOrderLineRequest {
                    inventory_item_id: id(ITEM_A),
                    quantity: "1.000000".to_owned(),
                    unit_cost: Some("0.0000".to_owned()),
                }],
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(order.lines[0].unit_cost.as_deref(), Some("0.0000"));
    assert_eq!(order.total.as_deref(), Some("0.0000000000"));
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn line_edits_require_draft_and_issue_is_irreversible()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-2").await;
    let line_id = id(&order.lines[0].id);

    let edited = app
        .update_purchase_order_line(
            &user,
            &mut ids,
            &id(&order.id),
            &line_id,
            UpdatePurchaseOrderLineRequest {
                expected_version: 0,
                quantity: "12.000000".to_owned(),
                unit_cost: Some(Some("2.7500".to_owned())),
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(edited.lines[0].quantity, "12.000000");
    assert_eq!(edited.version, "1");

    // Cambio de proveedor en borrador con CAS: re-captura snapshots.
    let second = app
        .create_supplier(&user, &mut ids, new_supplier("proveedor-2"), &at(TIME))
        .await?;
    let changed = app
        .change_purchase_order_supplier(
            &user,
            &mut ids,
            &id(&order.id),
            ChangeSupplierRequest {
                expected_version: 1,
                supplier_id: id(&second.id),
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(changed.supplier_id, second.id);
    assert_eq!(changed.supplier_code, "proveedor-2");
    assert_eq!(changed.version, "2");

    let issued = app
        .issue_purchase_order(&user, &mut ids, &id(&order.id), 2, &at(TIME))
        .await?;
    assert_eq!(issued.status, "ORDERED");
    assert_eq!(issued.version, "3");

    // Editar tras emitir falla y no toca la orden.
    assert_eq!(
        app.add_purchase_order_line(
            &user,
            &mut ids,
            &id(&order.id),
            NewPurchaseOrderLineRequest {
                inventory_item_id: id(ITEM_A),
                quantity: "1.000000".to_owned(),
                unit_cost: None,
            },
            &at(LATER),
        )
        .await
        .err(),
        Some(ApplicationError::PurchaseOrderNotDraft)
    );
    // Emitir dos veces falla.
    assert_eq!(
        app.issue_purchase_order(&user, &mut ids, &id(&order.id), 3, &at(LATER))
            .await
            .err(),
        Some(ApplicationError::PurchaseOrderNotDraft)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn stale_version_is_rejected_and_cancel_is_final() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-3").await;

    assert_eq!(
        app.issue_purchase_order(&user, &mut ids, &id(&order.id), 99, &at(TIME))
            .await
            .err(),
        Some(ApplicationError::ConcurrentModification)
    );
    let cancelled = app
        .cancel_purchase_order(&user, &mut ids, &id(&order.id), 0, &at(TIME))
        .await?;
    assert_eq!(cancelled.status, "CANCELLED");
    assert_eq!(
        app.cancel_purchase_order(&user, &mut ids, &id(&order.id), 1, &at(LATER))
            .await
            .err(),
        Some(ApplicationError::PurchaseOrderAlreadyCancelled)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn receipt_moves_stock_and_ledger_in_one_commit() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-4").await;
    let issued = app
        .issue_purchase_order(&user, &mut ids, &id(&order.id), 0, &at(TIME))
        .await?;
    let line_id = id(&issued.lines[0].id);

    let receipt = app
        .post_purchase_receipt(
            &user,
            &mut ids,
            &id(&order.id),
            PostReceiptRequest {
                expected_version: 1,
                idempotency_key: "receipt-1".to_owned(),
                lines: vec![ReceiptLineInput {
                    purchase_order_line_id: line_id.clone(),
                    quantity: "4.000000".to_owned(),
                }],
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(receipt.lines.len(), 1);

    let status = app.purchase_receipt_status(&user, &id(&order.id)).await?;
    assert_eq!(status[0].received_quantity, "4.000000");
    assert_eq!(status[0].pending_quantity, "6.000000");
    assert!(!status[0].fully_received);

    let order_now = app.get_purchase_order(&user, &id(&order.id)).await?;
    assert_eq!(order_now.status, "PARTIALLY_RECEIVED");

    let balance: String = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?",
    )
    .bind(ITEM_A)
    .fetch_one(&pool)
    .await?;
    assert_eq!(balance, "4.000000");
    let movements: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_ledger WHERE movement_type = 'PURCHASE_RECEIPT'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(movements, 1);
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_event WHERE action = 'purchase.receipt.posted'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(audits, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn receipt_is_idempotent_and_over_receipt_is_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-5").await;
    let issued = app
        .issue_purchase_order(&user, &mut ids, &id(&order.id), 0, &at(TIME))
        .await?;
    let line_id = id(&issued.lines[0].id);
    let request = || PostReceiptRequest {
        expected_version: 1,
        idempotency_key: "receipt-dup".to_owned(),
        lines: vec![ReceiptLineInput {
            purchase_order_line_id: line_id.clone(),
            quantity: "10.000000".to_owned(),
        }],
    };

    let first = app
        .post_purchase_receipt(&user, &mut ids, &id(&order.id), request(), &at(TIME))
        .await?;
    // El reintento devuelve el mismo hecho y no mueve stock otra vez.
    let second = app
        .post_purchase_receipt(&user, &mut ids, &id(&order.id), request(), &at(LATER))
        .await?;
    assert_eq!(first.id, second.id);
    let mut different_line = request();
    different_line.lines[0].purchase_order_line_id = id("00000000-0000-0000-0000-000000002d01");
    assert_eq!(
        app.post_purchase_receipt(&user, &mut ids, &id(&order.id), different_line, &at(LATER))
            .await
            .err(),
        Some(ApplicationError::DuplicateReceipt)
    );
    // El actor existente de B recibe permiso propio: el rechazo debe probar
    // aislamiento, no limitarse a la ausencia de grants de la fixture original.
    let other_role = "00000000-0000-0000-0000-000000002db0";
    sqlx::query("INSERT INTO role SELECT ?,?,code,name,status,version,created_at,updated_at FROM role WHERE id = ?")
        .bind(other_role).bind(ORG_B).bind(ROLE_A).execute(&pool).await?;
    sqlx::query("INSERT INTO role_permission (role_id,permission_code) VALUES (?,'purchasing.receipt.write')")
        .bind(other_role).execute(&pool).await?;
    sqlx::query("INSERT INTO user_organization_role (organization_id,user_id,role_id,created_at) VALUES (?,?,?,?)")
        .bind(ORG_B).bind(USER_B).bind(other_role).bind(TIME).execute(&pool).await?;
    let other_organization = principal(&pool, HASH_B, &id(BRANCH_OTHER)).await?;
    assert_eq!(
        app.post_purchase_receipt(
            &other_organization,
            &mut ids,
            &id(&order.id),
            request(),
            &at(LATER)
        )
        .await
        .err(),
        Some(ApplicationError::PurchaseOrderNotFound)
    );
    let mut different_payload = request();
    different_payload.lines[0].quantity = "1.000000".to_owned();
    assert_eq!(
        app.post_purchase_receipt(
            &user,
            &mut ids,
            &id(&order.id),
            different_payload,
            &at(LATER)
        )
        .await
        .err(),
        Some(ApplicationError::DuplicateReceipt)
    );
    assert_eq!(
        app.post_purchase_receipt(
            &user,
            &mut ids,
            &id("00000000-0000-0000-0000-000000002c01"),
            request(),
            &at(LATER)
        )
        .await
        .err(),
        Some(ApplicationError::DuplicateReceipt)
    );
    let other_scope = principal(&pool, HASH_A, &id(BRANCH_OTHER)).await?;
    assert_eq!(
        app.post_purchase_receipt(
            &other_scope,
            &mut ids,
            &id(&order.id),
            request(),
            &at(LATER)
        )
        .await
        .err(),
        Some(ApplicationError::Forbidden)
    );
    let balance: String = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?",
    )
    .bind(ITEM_A)
    .fetch_one(&pool)
    .await?;
    assert_eq!(balance, "10.000000");
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM purchase_receipt")
        .fetch_one(&pool)
        .await?;
    assert_eq!(receipts, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM inventory_ledger WHERE movement_type = 'PURCHASE_RECEIPT'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_event WHERE action = 'purchase.receipt.posted'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );

    // La orden quedó completa y ya no admite más recepciones.
    let order_now = app.get_purchase_order(&user, &id(&order.id)).await?;
    assert_eq!(order_now.status, "RECEIVED");
    assert_eq!(
        app.post_purchase_receipt(
            &user,
            &mut ids,
            &id(&order.id),
            PostReceiptRequest {
                expected_version: 2,
                idempotency_key: "receipt-2".to_owned(),
                lines: vec![ReceiptLineInput {
                    purchase_order_line_id: line_id.clone(),
                    quantity: "1.000000".to_owned(),
                }],
            },
            &at(LATER),
        )
        .await
        .err(),
        Some(ApplicationError::PurchaseOrderNotReceivable)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn over_receipt_within_a_valid_order_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-6").await;
    let issued = app
        .issue_purchase_order(&user, &mut ids, &id(&order.id), 0, &at(TIME))
        .await?;
    let line_id = id(&issued.lines[0].id);
    assert_eq!(
        app.post_purchase_receipt(
            &user,
            &mut ids,
            &id(&order.id),
            PostReceiptRequest {
                expected_version: 1,
                idempotency_key: "receipt-over".to_owned(),
                lines: vec![ReceiptLineInput {
                    purchase_order_line_id: line_id,
                    quantity: "11.000000".to_owned(),
                }],
            },
            &at(TIME),
        )
        .await
        .err(),
        Some(ApplicationError::OverReceipt)
    );
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM purchase_receipt")
        .fetch_one(&pool)
        .await?;
    assert_eq!(receipts, 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn receipt_requires_an_issued_order() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-7").await;
    assert_eq!(
        app.post_purchase_receipt(
            &user,
            &mut ids,
            &id(&order.id),
            PostReceiptRequest {
                expected_version: 0,
                idempotency_key: "receipt-draft".to_owned(),
                lines: vec![ReceiptLineInput {
                    purchase_order_line_id: id(&order.lines[0].id),
                    quantity: "1.000000".to_owned(),
                }],
            },
            &at(TIME),
        )
        .await
        .err(),
        Some(ApplicationError::PurchaseOrderNotReceivable)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn audit_failure_rolls_back_the_whole_receipt() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-8").await;
    let issued = app
        .issue_purchase_order(&user, &mut ids, &id(&order.id), 0, &at(TIME))
        .await?;
    sqlx::query(
        "CREATE TRIGGER fail_audit BEFORE INSERT ON audit_event \
         BEGIN SELECT RAISE(ABORT, 'auditoría rechazada'); END",
    )
    .execute(&pool)
    .await?;
    let result = app
        .post_purchase_receipt(
            &user,
            &mut ids,
            &id(&order.id),
            PostReceiptRequest {
                expected_version: 1,
                idempotency_key: "receipt-fail".to_owned(),
                lines: vec![ReceiptLineInput {
                    purchase_order_line_id: id(&issued.lines[0].id),
                    quantity: "2.000000".to_owned(),
                }],
            },
            &at(TIME),
        )
        .await;
    assert_eq!(result.err(), Some(ApplicationError::Audit));
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM purchase_receipt")
        .fetch_one(&pool)
        .await?;
    let balance: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_balance")
        .fetch_one(&pool)
        .await?;
    let movements: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_ledger")
        .fetch_one(&pool)
        .await?;
    let order_now = app.get_purchase_order(&user, &id(&order.id)).await?;
    assert_eq!((receipts, balance, movements), (0, 0, 0));
    assert_eq!(order_now.status, "ORDERED");
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn cross_organization_order_is_invisible_and_concurrent_suppliers_do_not_duplicate()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let app = application(&pool);
    let user = principal(&pool, HASH_A, &id(BRANCH_A)).await?;
    let mut ids = FixtureIds(0);
    let supplier = seeded_supplier(&app, &user, &mut ids).await;
    let order = draft_order(&app, &user, &mut ids, &supplier, "oc-9").await;

    let foreign = principal(&pool, HASH_B, &id(BRANCH_OTHER)).await?;
    // Sin membresía de sucursal el principal falla cerrado antes de buscar.
    assert_eq!(
        app.get_purchase_order(&foreign, &id(&order.id)).await.err(),
        Some(ApplicationError::Forbidden)
    );

    // Dos altas simultáneas del mismo slug: una sola gana.
    let first = {
        let app = app.clone();
        let user = user.clone();
        tokio::spawn(async move {
            let mut ids = FixtureIds(100);
            app.create_supplier(&user, &mut ids, new_supplier("carrera"), &at(TIME))
                .await
        })
    };
    let second = {
        let app = app.clone();
        let user = user.clone();
        tokio::spawn(async move {
            let mut ids = FixtureIds(200);
            app.create_supplier(&user, &mut ids, new_supplier("carrera"), &at(TIME))
                .await
        })
    };
    let (left, right) = (first.await?, second.await?);
    let successes = [&left, &right]
        .iter()
        .filter(|result| result.is_ok())
        .count();
    assert_eq!(successes, 1, "exactamente un alta debe confirmarse");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM supplier WHERE code = 'carrera'")
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 1);
    pool.close().await;
    Ok(())
}
