//! Integración SQLite real de ventas candidatas y coordinación con turnos.
#![allow(clippy::panic)]

use std::time::Duration;

use tokio::sync::oneshot;

use pos_core::{
    organization_branch::ApplicationError,
    sales::{NewSale, Sale, SaleScope, SaleStatus},
};

use super::cash_store_tests::{
    BRANCH_A, BRANCH_OTHER, FixtureIds, HASH_A, HASH_B, LATER, ORG_A, TIME, application, at,
    create_register, id, open_candidate, open_shift, principal,
};
use super::*;
use crate::{
    cash::CloseCashShiftRequest,
    sales::{AddSaleItemRequest, NewSaleRequest, SqliteSalesApplication, UpdateSaleItemRequest},
};

const PRODUCT: &str = "00000000-0000-0000-0000-000000000310";
const GROUP: &str = "00000000-0000-0000-0000-000000000311";
const OPTION: &str = "00000000-0000-0000-0000-000000000312";

fn sales(pool: &SqlitePool) -> SqliteSalesApplication {
    SqliteSalesApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

async fn insert_product(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO product (id, organization_id, category_id, sku, barcode, name, description, unit_price, status, version, created_at, updated_at) \
         VALUES (?, ?, NULL, 'CAFE-1', NULL, 'Café', NULL, '12.3456', 'ACTIVE', 0, ?, ?)",
    )
    .bind(PRODUCT)
    .bind(ORG_A)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_modifier_configuration(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO modifier_group (id, organization_id, code, name, min_selections, max_selections, status, version, created_at, updated_at) \
         VALUES (?, ?, 'leche', 'Leche', 0, 1, 'ACTIVE', 0, ?, ?)",
    )
    .bind(GROUP)
    .bind(ORG_A)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO modifier_option (id, organization_id, modifier_group_id, code, name, price_delta, display_order, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, 'avena', 'Avena', '0.5000', 0, 'ACTIVE', 0, ?, ?)",
    )
    .bind(OPTION)
    .bind(ORG_A)
    .bind(GROUP)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO product_modifier_group (organization_id, product_id, modifier_group_id, display_order, created_at) VALUES (?, ?, ?, 0, ?)",
    )
    .bind(ORG_A)
    .bind(PRODUCT)
    .bind(GROUP)
    .bind(TIME)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn open_sale_items_guard_cancel_close_audit_and_reopen()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    insert_product(&pool).await?;
    insert_modifier_configuration(&pool).await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let cash = application(&pool);
    let sales = sales(&pool);
    let mut ids = FixtureIds(300);
    let register = create_register(&cash, &owner, &mut ids, "ventas").await?;
    let shift = open_shift(&cash, &owner, &mut ids, register.id().clone()).await?;

    let sale = sales
        .create_sale(
            &owner,
            &mut ids,
            NewSaleRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
            },
            &at(TIME),
        )
        .await?;
    let (quoted, item) = sales
        .add_sale_item(
            &owner,
            &mut ids,
            register.id().clone(),
            shift.id().clone(),
            sale.id(),
            AddSaleItemRequest {
                expected_sale_version: 0,
                product_id: id(PRODUCT),
                quantity: 3,
                modifier_option_ids: vec![id(OPTION)],
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(item.effective_unit_price().to_string(), "12.8456");
    assert_eq!(item.line_total().to_string(), "38.5368");
    assert_eq!(quoted.total_amount().to_string(), "38.5368");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sale_item_modifier WHERE sale_item_id = ?"
        )
        .bind(item.id().as_str())
        .fetch_one(&pool)
        .await?,
        1,
        "la opción se resuelve y congela como snapshot; el cliente no envía precios",
    );
    assert_eq!(
        cash.close_cash_shift(
            &owner,
            &mut ids,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "15.0000".to_owned()
            },
            &at(LATER),
        )
        .await,
        Err(ApplicationError::CashShiftHasOpenSales),
        "el adaptador real consulta sale.status=OPEN dentro de la UoW de cierre",
    );
    let cancelled = sales
        .cancel_sale(
            &owner,
            &mut ids,
            register.id().clone(),
            shift.id().clone(),
            sale.id(),
            quoted.revision(),
            &at(LATER),
        )
        .await?;
    assert_eq!(cancelled.status(), SaleStatus::Cancelled);
    let closed = cash
        .close_cash_shift(
            &owner,
            &mut ids,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "15.0000".to_owned(),
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(closed.status().as_str(), "CLOSED");

    let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM audit_event ORDER BY action")
        .fetch_all(&pool)
        .await?;
    assert!(actions.contains(&"sale.created".to_owned()));
    assert!(actions.contains(&"sale.item.added".to_owned()));
    assert!(actions.contains(&"sale.cancelled".to_owned()));
    pool.close().await;
    let reopened = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM sale WHERE id = ?")
            .bind(sale.id().as_str())
            .fetch_one(&reopened)
            .await?,
        "CANCELLED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM cash_shift WHERE id = ?")
            .bind(shift.id().as_str())
            .fetch_one(&reopened)
            .await?,
        "CLOSED"
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn sale_writes_are_scoped_cas_safe_and_audit_atomic() -> Result<(), Box<dyn std::error::Error>>
{
    let (_temp, pool) = open_candidate().await?;
    insert_product(&pool).await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let cash = application(&pool);
    let sales = sales(&pool);
    let mut ids = FixtureIds(360);
    let register = create_register(&cash, &owner, &mut ids, "cas").await?;
    let shift = open_shift(&cash, &owner, &mut ids, register.id().clone()).await?;
    let sale = sales
        .create_sale(
            &owner,
            &mut ids,
            NewSaleRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
            },
            &at(TIME),
        )
        .await?;
    let (_, item) = sales
        .add_sale_item(
            &owner,
            &mut ids,
            register.id().clone(),
            shift.id().clone(),
            sale.id(),
            AddSaleItemRequest {
                expected_sale_version: 0,
                product_id: id(PRODUCT),
                quantity: 1,
                modifier_option_ids: vec![],
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(
        sales
            .add_sale_item(
                &owner,
                &mut ids,
                register.id().clone(),
                shift.id().clone(),
                sale.id(),
                AddSaleItemRequest {
                    expected_sale_version: 0,
                    product_id: id(PRODUCT),
                    quantity: 1,
                    modifier_option_ids: vec![]
                },
                &at(TIME)
            )
            .await,
        Err(ApplicationError::ConcurrentModification),
    );
    assert_eq!(
        sales
            .get_sale(
                &owner,
                register.id().clone(),
                shift.id().clone(),
                &sale.id().clone()
            )
            .await?
            .1[0]
            .id(),
        item.id(),
    );
    let foreign = principal(&pool, HASH_B, &id(BRANCH_OTHER), false).await?;
    assert_eq!(
        sales
            .get_sale(
                &foreign,
                register.id().clone(),
                shift.id().clone(),
                sale.id()
            )
            .await,
        Err(ApplicationError::Forbidden),
        "una sesión de otra organización no puede revelar una venta scoped",
    );
    let (updated, updated_item) = sales
        .update_sale_item(
            &owner,
            &mut ids,
            register.id().clone(),
            shift.id().clone(),
            sale.id(),
            item.id(),
            UpdateSaleItemRequest {
                expected_sale_version: 1,
                quantity: 2,
                modifier_option_ids: vec![],
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(updated.total_amount().to_string(), "24.6912");
    assert_eq!(updated_item.quantity(), 2);
    let removed = sales
        .remove_sale_item(
            &owner,
            &mut ids,
            register.id().clone(),
            shift.id().clone(),
            sale.id(),
            item.id(),
            2,
            &at(LATER),
        )
        .await?;
    assert_eq!(removed.total_amount().to_string(), "0.0000");
    sqlx::query("DROP TABLE audit_event").execute(&pool).await?;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sale")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        sales
            .create_sale(
                &owner,
                &mut ids,
                NewSaleRequest {
                    cash_register_id: register.id().clone(),
                    cash_shift_id: shift.id().clone()
                },
                &at(LATER)
            )
            .await,
        Err(ApplicationError::Audit),
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sale")
            .fetch_one(&pool)
            .await?,
        before
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_real_sale_creation_and_close_cannot_leave_open_sale_under_closed_shift()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, first_pool) = open_candidate().await?;
    let first_owner = principal(&first_pool, HASH_A, &id(BRANCH_A), false).await?;
    let first_cash = application(&first_pool);
    let mut setup_ids = FixtureIds(400);
    let register =
        create_register(&first_cash, &first_owner, &mut setup_ids, "carrera-venta").await?;
    let shift = open_shift(
        &first_cash,
        &first_owner,
        &mut setup_ids,
        register.id().clone(),
    )
    .await?;
    let second_pool = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    let second_owner = principal(&second_pool, HASH_A, &id(BRANCH_A), false).await?;
    let first_sales = sales(&first_pool);
    let second_cash = application(&second_pool);
    let mut sale_ids = FixtureIds(420);
    let mut close_ids = FixtureIds(440);
    let created_at = at(TIME);
    let closed_at = at(LATER);
    let create = first_sales.create_sale(
        &first_owner,
        &mut sale_ids,
        NewSaleRequest {
            cash_register_id: register.id().clone(),
            cash_shift_id: shift.id().clone(),
        },
        &created_at,
    );
    let close = second_cash.close_cash_shift(
        &second_owner,
        &mut close_ids,
        register.id(),
        shift.id(),
        CloseCashShiftRequest {
            expected_version: 0,
            closing_declared_amount: "3.0000".to_owned(),
        },
        &closed_at,
    );
    let (created, closed) = tokio::join!(create, close);
    assert!(
        (created.is_ok() && closed == Err(ApplicationError::CashShiftHasOpenSales))
            || (created == Err(ApplicationError::CashShiftAlreadyClosed) && closed.is_ok()),
        "BEGIN IMMEDIATE serializa crear venta y cerrar turno",
    );
    let shift_status: String = sqlx::query_scalar("SELECT status FROM cash_shift WHERE id = ?")
        .bind(shift.id().as_str())
        .fetch_one(&first_pool)
        .await?;
    let open_sales: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sale WHERE cash_shift_id = ? AND status = 'OPEN'")
            .bind(shift.id().as_str())
            .fetch_one(&first_pool)
            .await?;
    assert!(
        shift_status == "OPEN" || open_sales == 0,
        "nunca hay OPEN bajo un turno CLOSED"
    );
    second_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn cancelled_sale_transaction_rolls_back_before_connection_reuse()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let cash = application(&pool);
    let mut ids = FixtureIds(480);
    let register = create_register(&cash, &owner, &mut ids, "cancelar-venta").await?;
    let shift = open_shift(&cash, &owner, &mut ids, register.id().clone()).await?;
    let sale_id = id("00000000-0000-0000-0000-0000000003f0");
    let sale = Sale::open(NewSale {
        id: sale_id.clone(),
        scope: SaleScope {
            organization_id: id(ORG_A),
            branch_id: id(BRANCH_A),
            cash_register_id: register.id().clone(),
            cash_shift_id: shift.id().clone(),
        },
        created_by_user_id: owner.user_id().clone(),
    });
    let uow = SqliteUnitOfWork::new(pool.clone());
    let (inserted, observed) = oneshot::channel();
    let task = tokio::spawn(async move {
        let mut transaction = uow.begin().await.map_err(|_| ApplicationError::Storage)?;
        transaction
            .insert_sale(&sale, TIME)
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
    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sale WHERE id = ?")
            .bind(sale_id.as_str())
            .fetch_one(&pool)
            .await?,
        0,
        "la cancelación de una UoW de ventas no deja una venta OPEN parcial",
    );
    Ok(())
}

#[tokio::test]
async fn physical_sale_scope_and_money_constraints_reject_direct_invalid_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    assert!(sqlx::query(
        "INSERT INTO sale (id, organization_id, branch_id, cash_register_id, cash_shift_id, created_by_user_id, status, total_amount, version, created_at, updated_at) \
         VALUES ('00000000-0000-0000-0000-0000000003ff', ?, ?, ?, ?, ?, 'OPEN', '1.00000', 0, ?, ?)",
    )
    .bind(ORG_A).bind(BRANCH_A).bind("00000000-0000-0000-0000-000000000399")
    .bind("00000000-0000-0000-0000-000000000398").bind("00000000-0000-0000-0000-0000000001a0")
    .bind(TIME).bind(TIME).execute(&pool).await.is_err(), "FK y decimal canónico son barreras físicas");
    Ok(())
}
