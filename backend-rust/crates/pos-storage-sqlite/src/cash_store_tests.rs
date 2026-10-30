//! Integración SQLite real de cajas y turnos sobre el esquema candidato.
//!
//! El fixture aplica las propuestas de manera explícita: `pos-local::migrate()`
//! no descubre estos archivos. La tabla `candidate_open_sale_guard` es exclusiva
//! de estas pruebas y demuestra el puerto de ventas mientras V13 Local todavía
//! no existe; no forma parte del DDL candidato ni es una venta de producto.
#![allow(clippy::panic)]

use std::time::Duration;

use tokio::sync::oneshot;

use pos_core::{
    audit::AuditOccurredAt,
    cash::CashShiftStatus,
    identity::{LocalPrincipalService, SessionHash, UtcSeconds, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        IsoCurrency, LocaleTag, NewBranch, NewOrganization, Organization, Timezone,
    },
};
use sqlx::SqlitePool;

use super::*;
use crate::{
    cash_store::{
        CashPage, CashRegisterUpdateRequest, CashStorageError, CloseCashShiftRequest,
        NewCashRegisterRequest, OpenCashShiftRequest, OpenSalesForCashShift, SqliteCashApplication,
    },
    identity_lab::SqliteIdentityLab,
};

const PROPOSALS: [&str; 8] = [
    include_str!("../../../proposals/local-auth/0002_installation_identity.proposed.sql"),
    include_str!("../../../proposals/local-auth/0003_user_credential_session.proposed.sql"),
    include_str!("../../../proposals/local-auth/0004_rbac_catalog.proposed.sql"),
    include_str!("../../../proposals/local-auth/0005_branch_membership.proposed.sql"),
    include_str!("../../../proposals/local-auth/0006_audit_event.proposed.sql"),
    include_str!("../../../proposals/local-auth/0007_bootstrap_record.proposed.sql"),
    include_str!("../../../proposals/local-cash/0009_cash_register.proposed.sql"),
    include_str!("../../../proposals/local-cash/0010_cash_shift.proposed.sql"),
];

const ORG_A: &str = "00000000-0000-0000-0000-000000000180";
const ORG_B: &str = "00000000-0000-0000-0000-000000000181";
const BRANCH_A: &str = "00000000-0000-0000-0000-000000000190";
const BRANCH_B: &str = "00000000-0000-0000-0000-000000000191";
const BRANCH_OTHER: &str = "00000000-0000-0000-0000-000000000192";
const USER_A: &str = "00000000-0000-0000-0000-0000000001a0";
const USER_B: &str = "00000000-0000-0000-0000-0000000001a1";
const ROLE_A: &str = "00000000-0000-0000-0000-0000000001b0";
const ROLE_B: &str = "00000000-0000-0000-0000-0000000001b1";
const SESSION_A: &str = "00000000-0000-0000-0000-0000000001d0";
const SESSION_B: &str = "00000000-0000-0000-0000-0000000001d1";
const HASH_A: &str = "1111111122222222333333334444444455555555666666667777777788888888";
const HASH_B: &str = "2222222233333333444444445555555566666666777777778888888899999999";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const NOW: i64 = 1_793_235_600;

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn at(value: &str) -> AuditOccurredAt {
    AuditOccurredAt::new(value).unwrap_or_else(|error| panic!("instante: {error}"))
}

fn organization(value: &str, code: &str) -> Organization {
    Organization::create(
        id(value),
        NewOrganization {
            name: DisplayName::new("Operación").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    )
}

struct FixtureIds(u32);
impl IdentityGenerator for FixtureIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        self.0 += 1;
        EntityId::parse(&format!("00000000-0000-0000-0000-{:012x}", 0x200 + self.0))
            .map_err(|_| ApplicationError::Storage)
    }
}

struct CandidateOpenSales;
impl OpenSalesForCashShift for CandidateOpenSales {
    fn has_open_sales(
        &self,
        transaction: &mut SqliteTransaction,
        cash_shift_id: &EntityId,
    ) -> impl std::future::Future<Output = Result<bool, CashStorageError>> + Send {
        let cash_shift_id = cash_shift_id.clone();
        async move {
            let exists: i64 = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM candidate_open_sale_guard WHERE cash_shift_id = ?)",
            )
            .bind(cash_shift_id.as_str())
            .fetch_one(transaction.connection())
            .await
            .map_err(|_| CashStorageError::Database)?;
            Ok(exists != 0)
        }
    }
}

fn application(pool: &SqlitePool) -> SqliteCashApplication<CandidateOpenSales> {
    SqliteCashApplication::new(SqliteUnitOfWork::new(pool.clone()), CandidateOpenSales)
}

async fn open_candidate() -> Result<(tempfile::TempDir, SqlitePool), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    migrate(&pool).await?;
    for statement in PROPOSALS {
        sqlx::raw_sql(statement).execute(&pool).await?;
    }
    sqlx::query(
        "CREATE TABLE candidate_open_sale_guard (cash_shift_id TEXT PRIMARY KEY NOT NULL) STRICT",
    )
    .execute(&pool)
    .await?;

    let store = SqliteOrganizationBranchStore::new(pool.clone());
    for (organization_id, code) in [(ORG_A, "operacion-a"), (ORG_B, "operacion-b")] {
        store
            .insert_organization(&organization(organization_id, code), TIME, TIME)
            .await?;
    }
    for (branch_id, organization_id, code) in [
        (BRANCH_A, ORG_A, "centro"),
        (BRANCH_B, ORG_A, "norte"),
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
    Ok((temp, pool))
}

async fn seed_security(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    for (user, organization_id, email) in [
        (USER_A, ORG_A, "caja-a@hiqnet.test"),
        (USER_B, ORG_B, "caja-b@hiqnet.test"),
    ] {
        sqlx::query(
            "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES (?, ?, 'Operadora', ?, 'ACTIVE', 0, ?, ?)",
        )
        .bind(user)
        .bind(organization_id)
        .bind(email)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    for (role, organization_id, code) in [
        (ROLE_A, ORG_A, "cash-admin"),
        (ROLE_B, ORG_B, "cash-reader"),
    ] {
        sqlx::query(
            "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, 'Rol', 'ACTIVE', 0, ?, ?)",
        )
        .bind(role)
        .bind(organization_id)
        .bind(code)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    for permission in [
        "cash-register.read",
        "cash-register.create",
        "cash-register.update",
        "cash-shift.read",
        "cash-shift.open",
        "cash-shift.close",
    ] {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(ROLE_A)
            .bind(permission)
            .execute(pool)
            .await?;
    }
    for (organization_id, user_id, role_id) in [(ORG_A, USER_A, ROLE_A), (ORG_B, USER_B, ROLE_B)] {
        sqlx::query(
            "INSERT INTO user_organization_role (organization_id, user_id, role_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(role_id)
        .bind(TIME)
        .execute(pool)
        .await?;
    }
    for (membership, branch_id) in [
        ("00000000-0000-0000-0000-0000000001c0", BRANCH_A),
        ("00000000-0000-0000-0000-0000000001c1", BRANCH_B),
    ] {
        sqlx::query(
            "INSERT INTO branch_membership (id, organization_id, branch_id, user_id, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 'ACTIVE', 0, ?, ?)",
        )
        .bind(membership)
        .bind(ORG_A)
        .bind(branch_id)
        .bind(USER_A)
        .bind(TIME)
        .bind(TIME)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO branch_membership_role (organization_id, membership_id, role_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(ORG_A)
        .bind(membership)
        .bind(ROLE_A)
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
    Ok(())
}

async fn principal(
    pool: &SqlitePool,
    hash: &str,
    branch_id: &EntityId,
    recovery: bool,
) -> Result<VerifiedPrincipal, Box<dyn std::error::Error>> {
    let mut connection = pool.acquire().await?;
    let hash = SessionHash::new(hash)?;
    let mut service = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection));
    let result = if recovery {
        service
            .authenticate_recovery(&hash, UtcSeconds::new(NOW)?, branch_id)
            .await
    } else {
        service
            .authenticate(&hash, UtcSeconds::new(NOW)?, Some(branch_id))
            .await
    };
    connection.close().await?;
    Ok(result?)
}

async fn create_register(
    app: &SqliteCashApplication<CandidateOpenSales>,
    principal: &VerifiedPrincipal,
    identities: &mut FixtureIds,
    code: &str,
) -> Result<pos_core::cash::CashRegister, ApplicationError> {
    app.create_cash_register(
        principal,
        identities,
        NewCashRegisterRequest {
            code: code.to_owned(),
            name: "Caja principal".to_owned(),
        },
        &at(TIME),
    )
    .await
}

async fn open_shift(
    app: &SqliteCashApplication<CandidateOpenSales>,
    principal: &VerifiedPrincipal,
    identities: &mut FixtureIds,
    register_id: EntityId,
) -> Result<pos_core::cash::CashShift, ApplicationError> {
    app.open_cash_shift(
        principal,
        identities,
        OpenCashShiftRequest {
            cash_register_id: register_id,
            opening_amount: "10.2500".to_owned(),
        },
        &at(TIME),
    )
    .await
}

#[tokio::test]
async fn create_open_close_audits_and_survives_reopen() -> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(0);

    let register = create_register(&app, &owner, &mut identities, " caja-1 ").await?;
    assert_eq!(register.code().as_str(), "caja-1");
    let shift = open_shift(&app, &owner, &mut identities, register.id().clone()).await?;
    assert_eq!(shift.opening_amount().to_string(), "10.2500");
    assert_eq!(
        app.get_open_cash_shift(&owner, register.id())
            .await?
            .map(|item| item.id().clone()),
        Some(shift.id().clone())
    );

    let closed = app
        .close_cash_shift(
            &owner,
            &mut identities,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "23.5000".to_owned(),
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(closed.status(), CashShiftStatus::Closed);
    assert_eq!(
        closed
            .closing_declared_amount()
            .map(|amount| amount.to_string()),
        Some("23.5000".to_owned())
    );
    assert!(
        app.get_open_cash_shift(&owner, register.id())
            .await?
            .is_none()
    );

    let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM audit_event ORDER BY action")
        .fetch_all(&pool)
        .await?;
    assert_eq!(
        actions,
        [
            "cash-register.created",
            "cash-shift.closed",
            "cash-shift.opened"
        ]
    );
    pool.close().await;
    let reopened = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cash_shift")
            .fetch_one(&reopened)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event")
            .fetch_one(&reopened)
            .await?,
        3
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn database_scope_unique_code_and_amount_constraints_hold()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(20);
    let register = create_register(&app, &owner, &mut identities, "caja-1").await?;
    assert_eq!(
        create_register(&app, &owner, &mut identities, "CAJA-1").await,
        Err(ApplicationError::CashRegisterCodeAlreadyExists)
    );

    // Mismo código, otra sucursal: permitido físicamente porque el owner es branch.
    sqlx::query(
        "INSERT INTO cash_register (id, organization_id, branch_id, code, name, status, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000002ff', ?, ?, 'caja-1', 'Otra', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG_A)
    .bind(BRANCH_B)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await?;
    // La FK compuesta evita declarar una caja de org B en una branch de org A.
    assert!(sqlx::query(
        "INSERT INTO cash_register (id, organization_id, branch_id, code, name, status, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000002fe', ?, ?, 'ajena', 'Ajena', 'ACTIVE', 0, ?, ?)",
    )
    .bind(ORG_B)
    .bind(BRANCH_A)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await
    .is_err());
    assert!(sqlx::query(
        "INSERT INTO cash_shift (id, organization_id, branch_id, cash_register_id, opened_by_user_id, status, opening_amount, opened_at, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000002fd', ?, ?, ?, ?, 'OPEN', '1.00000', ?, 0, ?, ?)",
    )
    .bind(ORG_A)
    .bind(BRANCH_A)
    .bind(register.id().as_str())
    .bind(USER_A)
    .bind(TIME)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await
    .is_err(), "cinco decimales no son un importe canónico");
    assert!(sqlx::query(
        "INSERT INTO cash_shift (id, organization_id, branch_id, cash_register_id, opened_by_user_id, status, opening_amount, opened_at, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000002fc', ?, ?, ?, ?, 'OPEN', '.0000', ?, 0, ?, ?)",
    )
    .bind(ORG_A)
    .bind(BRANCH_A)
    .bind(register.id().as_str())
    .bind(USER_A)
    .bind(TIME)
    .bind(TIME)
    .bind(TIME)
    .execute(&pool)
    .await
    .is_err(), "el CHECK debe rechazar un importe sin parte entera");
    Ok(())
}

#[tokio::test]
async fn concurrent_opens_from_two_pools_confirm_exactly_one()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, first_pool) = open_candidate().await?;
    let first_owner = principal(&first_pool, HASH_A, &id(BRANCH_A), false).await?;
    let first_app = application(&first_pool);
    let mut seed_ids = FixtureIds(40);
    let register = create_register(&first_app, &first_owner, &mut seed_ids, "carrera").await?;

    let second_pool = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    let second_owner = principal(&second_pool, HASH_A, &id(BRANCH_A), false).await?;
    let second_app = application(&second_pool);
    let mut first_ids = FixtureIds(50);
    let mut second_ids = FixtureIds(60);
    let request = |register_id: EntityId| OpenCashShiftRequest {
        cash_register_id: register_id,
        opening_amount: "0.0000".to_owned(),
    };
    let opened_at = at(TIME);
    let (first, second) = tokio::join!(
        first_app.open_cash_shift(
            &first_owner,
            &mut first_ids,
            request(register.id().clone()),
            &opened_at,
        ),
        second_app.open_cash_shift(
            &second_owner,
            &mut second_ids,
            request(register.id().clone()),
            &opened_at,
        ),
    );
    let successes = usize::from(first.is_ok()) + usize::from(second.is_ok());
    assert_eq!(successes, 1, "solo un turno OPEN puede confirmar");
    let failure = if first.is_err() {
        first.err()
    } else {
        second.err()
    };
    assert_eq!(
        failure,
        Some(ApplicationError::CashRegisterAlreadyHasOpenShift)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cash_shift WHERE status = 'OPEN'")
            .fetch_one(&first_pool)
            .await?,
        1
    );
    second_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn close_is_cas_non_idempotent_and_preserves_closed_history()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(70);
    let register = create_register(&app, &owner, &mut identities, "historia").await?;
    let first = open_shift(&app, &owner, &mut identities, register.id().clone()).await?;
    assert_eq!(
        app.close_cash_shift(
            &owner,
            &mut identities,
            register.id(),
            first.id(),
            CloseCashShiftRequest {
                expected_version: 1,
                closing_declared_amount: "1.0000".to_owned()
            },
            &at(LATER)
        )
        .await,
        Err(ApplicationError::ConcurrentModification)
    );
    app.close_cash_shift(
        &owner,
        &mut identities,
        register.id(),
        first.id(),
        CloseCashShiftRequest {
            expected_version: 0,
            closing_declared_amount: "1.0000".to_owned(),
        },
        &at(LATER),
    )
    .await?;
    assert_eq!(
        app.close_cash_shift(
            &owner,
            &mut identities,
            register.id(),
            first.id(),
            CloseCashShiftRequest {
                expected_version: 1,
                closing_declared_amount: "1.0000".to_owned()
            },
            &at(LATER)
        )
        .await,
        Err(ApplicationError::CashShiftAlreadyClosed)
    );
    let second = open_shift(&app, &owner, &mut identities, register.id().clone()).await?;
    app.close_cash_shift(
        &owner,
        &mut identities,
        register.id(),
        second.id(),
        CloseCashShiftRequest {
            expected_version: 0,
            closing_declared_amount: "2.0000".to_owned(),
        },
        &at(LATER),
    )
    .await?;
    let shifts = app
        .list_cash_shifts(
            &owner,
            register.id(),
            Some(CashShiftStatus::Closed),
            CashPage {
                limit: 10,
                offset: 0,
            },
        )
        .await?;
    assert_eq!(
        shifts.len(),
        2,
        "los turnos CLOSED son históricos e ilimitados"
    );
    Ok(())
}

#[tokio::test]
async fn sales_guard_and_audit_failure_rollback_a_close() -> Result<(), Box<dyn std::error::Error>>
{
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(90);
    let register = create_register(&app, &owner, &mut identities, "guardia").await?;
    let shift = open_shift(&app, &owner, &mut identities, register.id().clone()).await?;
    sqlx::query("INSERT INTO candidate_open_sale_guard (cash_shift_id) VALUES (?)")
        .bind(shift.id().as_str())
        .execute(&pool)
        .await?;
    assert_eq!(
        app.close_cash_shift(
            &owner,
            &mut identities,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "1.0000".to_owned()
            },
            &at(LATER)
        )
        .await,
        Err(ApplicationError::CashShiftHasOpenSales)
    );
    sqlx::query("DELETE FROM candidate_open_sale_guard WHERE cash_shift_id = ?")
        .bind(shift.id().as_str())
        .execute(&pool)
        .await?;
    sqlx::query("DROP TABLE audit_event").execute(&pool).await?;
    assert_eq!(
        app.close_cash_shift(
            &owner,
            &mut identities,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "1.0000".to_owned()
            },
            &at(LATER)
        )
        .await,
        Err(ApplicationError::Audit)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM cash_shift WHERE id = ?")
            .bind(shift.id().as_str())
            .fetch_one(&pool)
            .await?,
        "OPEN"
    );
    Ok(())
}

async fn candidate_create_open_sale(
    pool: &SqlitePool,
    organization_id: &EntityId,
    branch_id: &EntityId,
    register_id: &EntityId,
    shift_id: &EntityId,
) -> Result<(), ApplicationError> {
    let uow = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = uow.begin().await.map_err(|_| ApplicationError::Storage)?;
    let operation = async {
        let shift = transaction
            .get_cash_shift(organization_id, branch_id, register_id, shift_id)
            .await
            .map_err(|_| ApplicationError::Storage)?
            .ok_or(ApplicationError::CashShiftNotFound)?;
        if !shift.is_open() {
            return Err(ApplicationError::CashShiftAlreadyClosed);
        }
        sqlx::query("INSERT INTO candidate_open_sale_guard (cash_shift_id) VALUES (?)")
            .bind(shift_id.as_str())
            .execute(transaction.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        Ok(())
    }
    .await;
    match operation {
        Ok(()) => {
            transaction
                .commit()
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Ok(())
        }
        Err(error) => {
            transaction
                .rollback()
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Err(error)
        }
    }
}

#[tokio::test]
async fn concurrent_sale_creation_and_close_are_serialized()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, first_pool) = open_candidate().await?;
    let owner = principal(&first_pool, HASH_A, &id(BRANCH_A), false).await?;
    let first_app = application(&first_pool);
    let mut identities = FixtureIds(110);
    let register = create_register(&first_app, &owner, &mut identities, "serializada").await?;
    let shift = open_shift(&first_app, &owner, &mut identities, register.id().clone()).await?;
    let second_pool = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    let second_owner = principal(&second_pool, HASH_A, &id(BRANCH_A), false).await?;
    let second_app = application(&second_pool);
    let mut close_ids = FixtureIds(120);
    let organization_id = id(ORG_A);
    let branch_id = id(BRANCH_A);
    let closed_at = at(LATER);
    let sale = candidate_create_open_sale(
        &first_pool,
        &organization_id,
        &branch_id,
        register.id(),
        shift.id(),
    );
    let close = second_app.close_cash_shift(
        &second_owner,
        &mut close_ids,
        register.id(),
        shift.id(),
        CloseCashShiftRequest {
            expected_version: 0,
            closing_declared_amount: "5.0000".to_owned(),
        },
        &closed_at,
    );
    let (sale, close) = tokio::join!(sale, close);
    assert!(
        (sale.is_ok() && close == Err(ApplicationError::CashShiftHasOpenSales))
            || (sale == Err(ApplicationError::CashShiftAlreadyClosed) && close.is_ok()),
        "la venta y el cierre se serializan: no pueden dejar una venta OPEN bajo un turno CLOSED"
    );
    let status: String = sqlx::query_scalar("SELECT status FROM cash_shift WHERE id = ?")
        .bind(shift.id().as_str())
        .fetch_one(&first_pool)
        .await?;
    let sales: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM candidate_open_sale_guard WHERE cash_shift_id = ?",
    )
    .bind(shift.id().as_str())
    .fetch_one(&first_pool)
    .await?;
    assert!(status == "OPEN" || sales == 0);
    second_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn recovery_scope_can_close_after_deactivation_and_revoked_sessions_cannot_reenter()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let normal = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(140);
    let register = create_register(&app, &normal, &mut identities, "recuperacion").await?;
    let shift = open_shift(&app, &normal, &mut identities, register.id().clone()).await?;
    sqlx::query("UPDATE cash_register SET status = 'INACTIVE' WHERE id = ?")
        .bind(register.id().as_str())
        .execute(&pool)
        .await?;
    assert_eq!(
        app.open_cash_shift(
            &normal,
            &mut identities,
            OpenCashShiftRequest {
                cash_register_id: register.id().clone(),
                opening_amount: "0.0000".to_owned(),
            },
            &at(LATER),
        )
        .await,
        Err(ApplicationError::CashRegisterInactive)
    );
    sqlx::query("UPDATE branch SET status = 'INACTIVE' WHERE id = ?")
        .bind(BRANCH_A)
        .execute(&pool)
        .await?;
    assert_eq!(
        app.close_cash_shift(
            &normal,
            &mut identities,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "0.0000".to_owned(),
            },
            &at(LATER),
        )
        .await,
        Err(ApplicationError::Forbidden),
        "el cierre de recursos inactivos exige capacidad de recuperación"
    );
    let recovery = principal(&pool, HASH_A, &id(BRANCH_A), true).await?;
    app.close_cash_shift(
        &recovery,
        &mut identities,
        register.id(),
        shift.id(),
        CloseCashShiftRequest {
            expected_version: 0,
            closing_declared_amount: "0.0000".to_owned(),
        },
        &at(LATER),
    )
    .await?;
    sqlx::query("UPDATE local_session SET state = 'REVOKED', revoked_at = ? WHERE id = ?")
        .bind(LATER)
        .bind(SESSION_A)
        .execute(&pool)
        .await?;
    assert!(
        principal(&pool, HASH_A, &id(BRANCH_A), false)
            .await
            .is_err(),
        "una sesión revocada no construye un principal nuevo"
    );
    Ok(())
}

#[tokio::test]
async fn cancelled_cash_transaction_rolls_back_before_connection_reuse()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let (inserted, observed) = oneshot::channel();
    let uow = SqliteUnitOfWork::new(pool.clone());
    let register = pos_core::cash::CashRegister::create(pos_core::cash::NewCashRegister {
        id: id("00000000-0000-0000-0000-0000000002fb"),
        organization_id: id(ORG_A),
        branch_id: id(BRANCH_A),
        code: pos_core::catalog::CatalogCode::new("cancelada")
            .unwrap_or_else(|error| panic!("code: {error}")),
        name: DisplayName::new("Caja cancelada").unwrap_or_else(|error| panic!("name: {error}")),
    });
    let task = tokio::spawn(async move {
        let mut transaction = uow.begin().await.map_err(|_| ApplicationError::Storage)?;
        transaction
            .insert_cash_register(&register, TIME)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        inserted.send(()).map_err(|_| ApplicationError::Storage)?;
        std::future::pending::<()>().await;
        #[allow(unreachable_code)]
        transaction
            .commit()
            .await
            .map_err(|_| ApplicationError::Storage)
    });
    observed.await?;
    task.abort();
    let join_result = task.await;
    assert!(matches!(join_result, Err(error) if error.is_cancelled()));
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cash_register WHERE code = 'cancelada'")
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 0, "la transacción abortada no deja una caja parcial");
    Ok(())
}

#[tokio::test]
async fn cross_organization_principal_is_denied_and_page_bounds_are_checked()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let foreign = principal(&pool, HASH_B, &id(BRANCH_OTHER), false).await?;
    let app = application(&pool);
    let mut identities = FixtureIds(160);
    let register = create_register(&app, &owner, &mut identities, "privada").await?;
    assert_eq!(
        app.get_cash_register(&foreign, register.id()).await,
        Err(ApplicationError::Forbidden)
    );
    assert_eq!(
        app.list_cash_registers(
            &owner,
            None,
            CashPage {
                limit: 0,
                offset: 0
            }
        )
        .await,
        Err(ApplicationError::InvalidCashInput)
    );
    let updated = app
        .update_cash_register(
            &owner,
            &mut identities,
            register.id(),
            CashRegisterUpdateRequest {
                expected_version: 0,
                name: Some("Caja editada".to_owned()),
                status: None,
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(updated.revision(), 1);
    Ok(())
}
