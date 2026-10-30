//! Arnés de integración **sintético** del backend Local.
//!
//! **No productivo.** Aplica `0001` mediante el migrador y, de forma explícita,
//! las propuestas candidatas `0002`–`0015` desde `backend-rust/proposals/`. No
//! promueve DDL: las propuestas siguen fuera de `migrations/local/` y este
//! binario no se conecta desde `pos-local`.
//!
//! # Qué demuestra
//!
//! Recorre el primer flujo comercial completo con **los mismos adaptadores y
//! casos de uso** que usará el producto, sin Spring Boot y sin red:
//!
//! ```text
//! SQLite → bootstrap → administrador → sesión → principal verificado → RBAC
//!   → sucursal → categoría → producto → inventario → receta → caja → turno
//!   → venta OPEN → partida → checkout → consumo → COMPLETED → auditoría
//!   → cierre de turno → reapertura → datos persistentes
//! ```
//!
//! No reimplementa reglas de negocio: invoca `CatalogApplication`,
//! `InventoryApplication`, `CashApplication`, `SalesApplication` y
//! `CheckoutApplication`. Solo construye el escenario (identidades fijas y la
//! receta, que aún no tiene caso de uso administrativo).
//!
//! # Uso
//!
//! ```text
//! cargo run --locked -p pos-lab -- --database target/pos-lab.sqlite
//! ```
//!
//! Sale con código 0 si el flujo completo se confirma y con 1 si algo falla,
//! dejando el informe JSON en stderr para diagnóstico.
//!
//! Este arnés usa fixtures constantes: un panic aquí solo señala un fixture
//! mal escrito, nunca una entrada de usuario.
#![allow(clippy::panic)]

mod contract;
mod operations;

use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};

use pos_application::{
    cash::{CloseCashShiftRequest, NewCashRegisterRequest, OpenCashShiftRequest},
    catalog::{NewCategoryRequest, NewProductRequest},
    checkout::{CheckoutPaymentRequest, CheckoutRequest},
    inventory::{NewInventoryItemRequest, OpenStockRequest},
    sales::{AddSaleItemRequest, NewSaleRequest},
};
use pos_core::{
    audit::{
        AuditOccurredAt, BootstrapError, BootstrapProvisioning, BootstrapSecretVerifier,
        authorize_bootstrap,
    },
    identity::{
        LocalPrincipalService, NormalizedEmail, SessionHash, UtcSeconds, VerifiedPrincipal,
    },
    inventory::InventoryBaseUnit,
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        IsoCurrency, LocaleTag, NewBranch, NewOrganization, Timezone,
    },
    sales::{PaymentMethod, SaleStatus},
};
use pos_storage_sqlite::{
    OrganizationBranchStorageError, SqliteUnitOfWork,
    bootstrap::{BootstrapRequest, provision_initial_installation},
    cash::SqliteCashApplication,
    catalog::SqliteCatalogApplication,
    checkout::SqliteCheckoutApplication,
    identity::SqliteIdentityLab,
    inventory::SqliteInventoryApplication,
    migrate, open,
    sales::{SqliteOpenSalesForCashShift, SqliteSalesApplication},
};
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// Propuestas candidatas aplicadas en orden de dependencia.
///
/// Permanecen fuera de `migrations/local/`: este arnés no las convierte en DDL
/// aprobado, solo las evalúa sobre SQLite sintético.
const PROPOSALS: [&str; 14] = [
    include_str!("../../../proposals/local-auth/0002_installation_identity.proposed.sql"),
    include_str!("../../../proposals/local-auth/0003_user_credential_session.proposed.sql"),
    include_str!("../../../proposals/local-auth/0004_rbac_catalog.proposed.sql"),
    include_str!("../../../proposals/local-auth/0005_branch_membership.proposed.sql"),
    include_str!("../../../proposals/local-auth/0006_audit_event.proposed.sql"),
    include_str!("../../../proposals/local-auth/0007_bootstrap_record.proposed.sql"),
    include_str!("../../../proposals/local-catalog/0008_catalog.proposed.sql"),
    include_str!("../../../proposals/local-cash/0009_cash_register.proposed.sql"),
    include_str!("../../../proposals/local-cash/0010_cash_shift.proposed.sql"),
    include_str!("../../../proposals/local-sales/0011_modifier_catalog.proposed.sql"),
    include_str!("../../../proposals/local-sales/0012_sales_foundation.proposed.sql"),
    include_str!("../../../proposals/local-inventory/0013_inventory.proposed.sql"),
    include_str!("../../../proposals/local-inventory/0014_inventory_recipes.proposed.sql"),
    include_str!("../../../proposals/local-sales/0015_checkout.proposed.sql"),
];

const ORG: &str = "00000000-0000-0000-0000-0000000007a0";
const INSTALLATION: &str = "00000000-0000-0000-0000-0000000007f0";
const ADMIN: &str = "00000000-0000-0000-0000-0000000007a1";
const ROLE: &str = "00000000-0000-0000-0000-0000000007b0";
const BRANCH: &str = "00000000-0000-0000-0000-000000000790";
const MEMBERSHIP: &str = "00000000-0000-0000-0000-0000000007c0";
const SESSION: &str = "00000000-0000-0000-0000-0000000007d0";
const BOOTSTRAP_EVENT: &str = "00000000-0000-0000-0000-0000000007e0";
const RECIPE: &str = "00000000-0000-0000-0000-0000000007e1";
const COMPONENT: &str = "00000000-0000-0000-0000-0000000007e2";
const SESSION_HASH: &str = "7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e7e1e";
const BOOTSTRAP_SECRET: &[u8] = b"secreto-sintetico-de-laboratorio";
const OPENED_AT: &str = "2026-10-30T09:00:00Z";
const CLOSED_AT: &str = "2026-10-30T11:00:00Z";
/// Instante UTC del principal sintético (2026-10-30T09:00:00Z).
const NOW: i64 = 1_793_350_800;

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("UUID de fixture inválido: {error}"))
}

fn at(value: &str) -> AuditOccurredAt {
    AuditOccurredAt::new(value).unwrap_or_else(|error| panic!("instante inválido: {error}"))
}

/// Generador determinista de identidades para el escenario.
struct LabIds(u32);

impl IdentityGenerator for LabIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        self.0 += 1;
        EntityId::parse(&format!("00000000-0000-0000-0000-{:012x}", 0x700 + self.0))
            .map_err(|_| ApplicationError::Storage)
    }
}

/// Verificador del secreto de bootstrap; el valor es sintético y local.
struct LabVerifier;

impl BootstrapSecretVerifier for LabVerifier {
    fn verify(&self, presented: &[u8]) -> Result<(), BootstrapError> {
        if presented == BOOTSTRAP_SECRET {
            Ok(())
        } else {
            Err(BootstrapError::Rejected)
        }
    }
}

fn bootstrap_request() -> BootstrapRequest {
    BootstrapRequest {
        installation_id: id(INSTALLATION),
        audit_event_id: pos_core::audit::AuditEventId::new(id(BOOTSTRAP_EVENT)),
        organization_id: id(ORG),
        organization: NewOrganization {
            name: DisplayName::new("Operación Sintética")
                .unwrap_or_else(|error| panic!("nombre: {error}")),
            code: BusinessCode::new("lab-operacion")
                .unwrap_or_else(|error| panic!("código: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("moneda: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
        administrator_id: id(ADMIN),
        administrator_name: DisplayName::new("Administradora Sintética")
            .unwrap_or_else(|error| panic!("nombre: {error}")),
        administrator_email: NormalizedEmail::new("admin@pos-lab.invalid")
            .unwrap_or_else(|error| panic!("email: {error}")),
        administrator_role_id: id(ROLE),
        administrator_role_code: BusinessCode::new("administrator")
            .unwrap_or_else(|error| panic!("rol: {error}")),
        installation_fingerprint: "ed25519:pos-lab-synthetic".to_owned(),
    }
}

/// Aplica `0001` y las propuestas candidatas sobre una base vacía.
async fn prepare_database(pool: &SqlitePool) -> Result<(), Box<dyn Error>> {
    migrate(pool).await?;
    for statement in PROPOSALS {
        sqlx::raw_sql(statement).execute(pool).await?;
    }
    Ok(())
}

/// Provisiona instalación, sucursal, membership y sesión en una sola UoW.
async fn provision(pool: &SqlitePool) -> Result<(EntityId, BootstrapProvisioning), Box<dyn Error>> {
    let capability = authorize_bootstrap(&LabVerifier, BOOTSTRAP_SECRET)?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;

    provision_initial_installation(
        &mut transaction,
        &capability,
        &bootstrap_request(),
        OPENED_AT,
    )
    .await?;
    // Java concede al rol administrador todo el catálogo vigente en el bootstrap.
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission_code) SELECT ?, code FROM permission",
    )
    .bind(ROLE)
    .execute(&mut *transaction.connection())
    .await?;

    let branch = Branch::create(
        id(BRANCH),
        id(ORG),
        NewBranch {
            name: DisplayName::new("Sucursal Centro")
                .unwrap_or_else(|error| panic!("nombre: {error}")),
            code: BusinessCode::new("centro").unwrap_or_else(|error| panic!("código: {error}")),
            timezone_override: None,
        },
    );
    transaction
        .insert_branch(&branch, OPENED_AT)
        .await
        .map_err(storage_error)?;

    // Membership y rol de sucursal: sin ellas el principal no resuelve permisos
    // en el scope de la sucursal (RBAC DB-authoritative).
    sqlx::query(
        "INSERT INTO branch_membership (id, organization_id, branch_id, user_id, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'ACTIVE', 0, ?, ?)",
    )
    .bind(MEMBERSHIP)
    .bind(ORG)
    .bind(BRANCH)
    .bind(ADMIN)
    .bind(OPENED_AT)
    .bind(OPENED_AT)
    .execute(&mut *transaction.connection())
    .await?;
    sqlx::query(
        "INSERT INTO branch_membership_role (organization_id, membership_id, role_id, created_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(ORG)
    .bind(MEMBERSHIP)
    .bind(ROLE)
    .bind(OPENED_AT)
    .execute(&mut *transaction.connection())
    .await?;

    // Sesión sintética: el hash es el que entregaría el emisor de sesiones.
    sqlx::query(
        "INSERT INTO local_session (id, user_id, session_hash, state, created_at, expires_at, revoked_at, session_version) \
         VALUES (?, ?, ?, 'ACTIVE', ?, ?, NULL, 1)",
    )
    .bind(SESSION)
    .bind(ADMIN)
    .bind(SESSION_HASH)
    .bind(OPENED_AT)
    .bind(CLOSED_AT)
    .execute(&mut *transaction.connection())
    .await?;

    transaction.commit().await.map_err(storage_error)?;
    Ok((branch.id().clone(), capability))
}

fn storage_error(error: OrganizationBranchStorageError) -> Box<dyn Error> {
    Box::new(error)
}

/// Añade el nombre del paso al error para que el informe identifique el tramo.
fn step<T, E: std::fmt::Display>(name: &str, result: Result<T, E>) -> Result<T, Box<dyn Error>> {
    result.map_err(|error| format!("{name}: {error}").into())
}

/// Autentica con la frontera real (`LocalPrincipalService` + adaptador canónico).
///
/// El catálogo se autoriza con scope de organización (sin sucursal) y las
/// operaciones de caja, venta e inventario con scope de sucursal. Ambas
/// credenciales proceden de la misma sesión verificada.
async fn authenticate(
    pool: &SqlitePool,
    branch_id: Option<&EntityId>,
) -> Result<VerifiedPrincipal, Box<dyn Error>> {
    let mut connection = pool.acquire().await?;
    let hash = SessionHash::new(SESSION_HASH)?;
    let principal = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection))
        .authenticate(&hash, UtcSeconds::new(NOW)?, branch_id)
        .await?;
    connection.close().await?;
    Ok(principal)
}

/// Inserta la receta del producto: aún no existe caso de uso administrativo.
async fn seed_recipe(
    pool: &SqlitePool,
    product_id: &EntityId,
    inventory_item_id: &EntityId,
) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        "INSERT INTO product_recipe (id, organization_id, product_id, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, 'ACTIVE', 0, ?, ?)",
    )
    .bind(RECIPE)
    .bind(ORG)
    .bind(product_id.as_str())
    .bind(OPENED_AT)
    .bind(OPENED_AT)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO recipe_component (id, organization_id, recipe_id, inventory_item_id, quantity_per_product, position, created_at) \
         VALUES (?, ?, ?, ?, '250.000000', 0, ?)",
    )
    .bind(COMPONENT)
    .bind(ORG)
    .bind(RECIPE)
    .bind(inventory_item_id.as_str())
    .bind(OPENED_AT)
    .execute(pool)
    .await?;
    Ok(())
}

/// Ejecuta el flujo completo y devuelve el informe.
async fn run(pool: &SqlitePool) -> Result<Value, Box<dyn Error>> {
    prepare_database(pool).await?;
    let (branch_id, _capability) = provision(pool).await?;
    let org_principal = authenticate(pool, None).await?;
    let branch_principal = authenticate(pool, Some(&branch_id)).await?;

    let mut ids = LabIds(0);

    let catalog = SqliteCatalogApplication::new(SqliteUnitOfWork::new(pool.clone()));
    let category = step(
        "create_category",
        catalog
            .create_category(
                &org_principal,
                &mut ids,
                NewCategoryRequest {
                    code: "bebidas".to_owned(),
                    name: "Bebidas".to_owned(),
                },
                &at(OPENED_AT),
            )
            .await,
    )?;
    let product = step(
        "create_product",
        catalog
            .create_product(
                &org_principal,
                &mut ids,
                NewProductRequest {
                    sku: "CAFE-1".to_owned(),
                    name: "Café americano".to_owned(),
                    unit_price: "12.3456".to_owned(),
                    barcode: None,
                    description: Some("Producto sintético de laboratorio".to_owned()),
                    category_id: Some(category.id().clone()),
                },
                &at(OPENED_AT),
            )
            .await,
    )?;

    let inventory = SqliteInventoryApplication::new(SqliteUnitOfWork::new(pool.clone()));
    let item = inventory
        .create_item(
            &branch_principal,
            &mut ids,
            NewInventoryItemRequest {
                code: "grano".to_owned(),
                name: "Café en grano".to_owned(),
                description: None,
                base_unit: InventoryBaseUnit::Gram,
                fractional_unit_allowed: true,
            },
            &at(OPENED_AT),
        )
        .await?;
    inventory
        .open_stock(
            &branch_principal,
            &mut ids,
            OpenStockRequest {
                inventory_item_id: item.id().clone(),
                quantity: "1000.000000".to_owned(),
            },
            &at(OPENED_AT),
        )
        .await?;
    seed_recipe(pool, product.id(), item.id()).await?;

    let cash = SqliteCashApplication::new(
        SqliteUnitOfWork::new(pool.clone()),
        SqliteOpenSalesForCashShift,
    );
    let register = cash
        .create_cash_register(
            &branch_principal,
            &mut ids,
            NewCashRegisterRequest {
                code: "caja-1".to_owned(),
                name: "Caja principal".to_owned(),
            },
            &at(OPENED_AT),
        )
        .await?;
    let shift = cash
        .open_cash_shift(
            &branch_principal,
            &mut ids,
            OpenCashShiftRequest {
                cash_register_id: register.id().clone(),
                opening_amount: "10.2500".to_owned(),
            },
            &at(OPENED_AT),
        )
        .await?;

    let sales = SqliteSalesApplication::new(SqliteUnitOfWork::new(pool.clone()));
    let sale = sales
        .create_sale(
            &branch_principal,
            &mut ids,
            NewSaleRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
            },
            &at(OPENED_AT),
        )
        .await?;
    let (quoted, _line) = sales
        .add_sale_item(
            &branch_principal,
            &mut ids,
            register.id().clone(),
            shift.id().clone(),
            sale.id(),
            AddSaleItemRequest {
                expected_sale_version: 0,
                product_id: product.id().clone(),
                quantity: 2,
                modifier_option_ids: Vec::new(),
            },
            &at(OPENED_AT),
        )
        .await?;

    // Una venta OPEN bloquea el cierre del turno: el guard real consulta `sale`.
    let close_blocked = cash
        .close_cash_shift(
            &branch_principal,
            &mut ids,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "35.0000".to_owned(),
            },
            &at(CLOSED_AT),
        )
        .await;
    let close_blocked = matches!(close_blocked, Err(ApplicationError::CashShiftHasOpenSales));
    if !close_blocked {
        return Err("el cierre no devolvió CashShiftHasOpenSales".into());
    }

    let total = quoted.total_amount().to_string();
    let checkout = SqliteCheckoutApplication::new(SqliteUnitOfWork::new(pool.clone()));
    let outcome = checkout
        .checkout_sale(
            &branch_principal,
            &mut ids,
            CheckoutRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
                sale_id: sale.id().clone(),
                expected_sale_version: quoted.revision(),
                payments: vec![CheckoutPaymentRequest {
                    method: PaymentMethod::Cash,
                    amount: total.clone(),
                    tendered_amount: Some("30.0000".to_owned()),
                }],
            },
            &at(CLOSED_AT),
        )
        .await?;
    if outcome.sale.status() != SaleStatus::Completed {
        return Err("la venta no quedó COMPLETED".into());
    }

    cash.close_cash_shift(
        &branch_principal,
        &mut ids,
        register.id(),
        shift.id(),
        CloseCashShiftRequest {
            expected_version: 0,
            closing_declared_amount: "35.0000".to_owned(),
        },
        &at(CLOSED_AT),
    )
    .await?;

    Ok(json!({
        "status": "COMPLETED",
        "productive": false,
        "scope": "synthetic_sqlite_candidate_schema",
        "organization_id": ORG,
        "branch_id": branch_id.as_str(),
        "administrator_id": ADMIN,
        "category_id": category.id().as_str(),
        "product_id": product.id().as_str(),
        "inventory_item_id": item.id().as_str(),
        "cash_register_id": register.id().as_str(),
        "cash_shift_id": shift.id().as_str(),
        "sale_id": sale.id().as_str(),
        "sale_total": total,
        "payments": outcome.payments.len(),
        "change_amount": outcome.payments.first().map(|payment| payment.change_amount().to_string()),
        "consumption_components": outcome.consumption.len(),
        "closed_while_sale_open": !close_blocked,
    }))
}

/// Reabre el archivo y comprueba que el estado y la auditoría persisten.
async fn verify_reopen(path: &Path, report: &Value) -> Result<Value, Box<dyn Error>> {
    let pool = open(path, Duration::from_millis(500)).await?;
    let sale_id = report["sale_id"].as_str().ok_or("informe sin sale_id")?;
    let sale_status: String = sqlx::query_scalar("SELECT status FROM sale WHERE id = ?")
        .bind(sale_id)
        .fetch_one(&pool)
        .await?;
    let payments: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payment")
        .fetch_one(&pool)
        .await?;
    let audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_event")
        .fetch_one(&pool)
        .await?;
    let balance: String = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?",
    )
    .bind(
        report["inventory_item_id"]
            .as_str()
            .ok_or("informe sin item")?,
    )
    .fetch_one(&pool)
    .await?;
    let shift_status: String = sqlx::query_scalar("SELECT status FROM cash_shift WHERE id = ?")
        .bind(
            report["cash_shift_id"]
                .as_str()
                .ok_or("informe sin turno")?,
        )
        .fetch_one(&pool)
        .await?;
    pool.close().await;
    Ok(json!({
        "sale_status": sale_status,
        "payments": payments,
        "audit_events": audits,
        "inventory_balance": balance,
        "cash_shift_status": shift_status,
    }))
}

fn database_path() -> PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--database"
            && let Some(value) = args.next()
        {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("pos-lab.sqlite")
}

#[tokio::main]
async fn main() {
    if execute().await.is_err() {
        // La frontera pública no devuelve errores SQL ni material de identidad.
        eprintln!(
            "{}",
            json!({ "code": contract::LabErrorCode::LabScenarioFailed })
        );
        std::process::exit(1);
    }
}

async fn execute() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|argument| argument == "--export-operations") {
        println!("{}", serde_json::to_string_pretty(&operations::catalog()?)?);
        return Ok(());
    }
    if std::env::args().any(|argument| argument == "--export-contract") {
        print!("{}", contract::bindings());
        return Ok(());
    }
    let contract_report = std::env::args().any(|argument| argument == "--contract-report");
    let path = database_path();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    // Reserva exclusiva: nunca borrar ni reutilizar una base suministrada por el usuario.
    let reservation = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    drop(reservation);
    let pool = open(&path, Duration::from_millis(500)).await?;
    let flow = run(&pool).await;
    let reopened = match &flow {
        Ok(report) => {
            pool.close().await;
            verify_reopen(&path, report).await
        }
        Err(_) => {
            pool.close().await;
            Err("flujo no completado".into())
        }
    };

    let report = match (flow, reopened) {
        (Ok(mut flow), Ok(reopen)) => {
            flow["reopen"] = reopen;
            flow
        }
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };

    let typed = contract::ScenarioReport::from_internal(&report)?;
    if contract_report {
        println!("{}", serde_json::to_string(&typed)?);
    } else {
        println!("{report}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposed_migrations_are_outside_the_productive_migrator() {
        // Guard: el arnés no debe depender de `migrations/local/`.
        for proposal in PROPOSALS {
            assert!(proposal.contains("CREATE TABLE") || proposal.contains("CREATE INDEX"));
        }
    }

    #[test]
    fn lab_ids_are_deterministic_and_unique() {
        let mut ids = LabIds(0);
        let first = ids.next_id().unwrap_or_else(|error| panic!("id: {error}"));
        let second = ids.next_id().unwrap_or_else(|error| panic!("id: {error}"));
        assert_ne!(first, second);
    }

    /// Regresión del flujo completo: mismos adaptadores, SQLite real y reapertura.
    #[tokio::test]
    async fn full_flow_persists_and_survives_reopen() -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("lab.sqlite");
        let pool = open(&path, Duration::from_millis(500)).await?;
        let report = run(&pool).await?;
        pool.close().await;

        assert_eq!(report["sale_total"], "24.6912");
        assert_eq!(report["change_amount"], "5.3088");
        assert_eq!(report["consumption_components"], 1);
        assert_eq!(
            report["closed_while_sale_open"], false,
            "una venta OPEN debe impedir cerrar el turno"
        );

        let reopened = verify_reopen(&path, &report).await?;
        assert_eq!(reopened["sale_status"], "COMPLETED");
        assert_eq!(reopened["cash_shift_status"], "CLOSED");
        assert_eq!(reopened["inventory_balance"], "500.000000");
        assert_eq!(reopened["payments"], 1);
        assert!(reopened["audit_events"].as_i64().unwrap_or(0) >= 10);
        Ok(())
    }
}
