//! Integración SQLite real del checkout candidato: pagos, consumo y auditoría.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_core::{
    inventory::InventoryBaseUnit,
    organization_branch::ApplicationError,
    sales::{PaymentMethod, SaleStatus},
};

use super::*;
use crate::{
    cash_store::{CloseCashShiftRequest, SqliteCashApplication},
    cash_store_tests::{
        BRANCH_A, FixtureIds, HASH_A, LATER, ORG_A, TIME, application, at, create_register, id,
        open_candidate, open_shift, principal,
    },
    checkout_store::{CheckoutPaymentRequest, CheckoutRequest, SqliteCheckoutApplication},
    inventory_store::{
        AdjustStockRequest, NewInventoryItemRequest, OpenStockRequest, SqliteInventoryApplication,
    },
    sales_store::{
        AddSaleItemRequest, NewSaleRequest, SqliteOpenSalesForCashShift, SqliteSalesApplication,
    },
};

const PRODUCT: &str = "00000000-0000-0000-0000-000000000410";
const GROUP: &str = "00000000-0000-0000-0000-000000000411";
const OPTION: &str = "00000000-0000-0000-0000-000000000412";
const RECIPE: &str = "00000000-0000-0000-0000-000000000413";
const COMPONENT: &str = "00000000-0000-0000-0000-000000000414";
const EFFECT: &str = "00000000-0000-0000-0000-000000000415";

fn checkout(pool: &SqlitePool) -> SqliteCheckoutApplication {
    SqliteCheckoutApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

fn sales(pool: &SqlitePool) -> SqliteSalesApplication {
    SqliteSalesApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

fn inventory(pool: &SqlitePool) -> SqliteInventoryApplication {
    SqliteInventoryApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

/// Siembra producto, modificador, artículo de inventario, receta y efecto.
async fn seed_recipe_product(
    pool: &SqlitePool,
    owner: &pos_core::identity::VerifiedPrincipal,
    ids: &mut FixtureIds,
) -> Result<pos_core::organization_branch::EntityId, Box<dyn std::error::Error>> {
    let item = inventory(pool)
        .create_item(
            owner,
            ids,
            NewInventoryItemRequest {
                code: "grano".to_owned(),
                name: "Café en grano".to_owned(),
                description: None,
                base_unit: InventoryBaseUnit::Gram,
                fractional_unit_allowed: true,
            },
            &at(TIME),
        )
        .await?;
    inventory(pool)
        .open_stock(
            owner,
            ids,
            OpenStockRequest {
                inventory_item_id: item.id().clone(),
                quantity: "1000.000000".to_owned(),
            },
            &at(TIME),
        )
        .await?;

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
    sqlx::query(
        "INSERT INTO product_recipe (id, organization_id, product_id, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, 'ACTIVE', 0, ?, ?)",
    )
    .bind(RECIPE)
    .bind(ORG_A)
    .bind(PRODUCT)
    .bind(TIME)
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO recipe_component (id, organization_id, recipe_id, inventory_item_id, quantity_per_product, position, created_at) \
         VALUES (?, ?, ?, ?, '250.000000', 0, ?)",
    )
    .bind(COMPONENT)
    .bind(ORG_A)
    .bind(RECIPE)
    .bind(item.id().as_str())
    .bind(TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO modifier_inventory_effect (id, organization_id, modifier_option_id, inventory_item_id, quantity_delta, created_at) \
         VALUES (?, ?, ?, ?, '50.000000', ?)",
    )
    .bind(EFFECT)
    .bind(ORG_A)
    .bind(OPTION)
    .bind(item.id().as_str())
    .bind(TIME)
    .execute(pool)
    .await?;
    Ok(item.id().clone())
}

/// Prepara una venta OPEN con una línea y su modificador.
async fn open_sale_with_item(
    pool: &SqlitePool,
    owner: &pos_core::identity::VerifiedPrincipal,
    ids: &mut FixtureIds,
) -> Result<
    (
        pos_core::cash::CashRegister,
        pos_core::cash::CashShift,
        pos_core::sales::Sale,
    ),
    Box<dyn std::error::Error>,
> {
    let cash = application(pool);
    let register = create_register(&cash, owner, ids, "checkout").await?;
    let shift = open_shift(&cash, owner, ids, register.id().clone()).await?;
    let sales = sales(pool);
    let sale = sales
        .create_sale(
            owner,
            ids,
            NewSaleRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
            },
            &at(TIME),
        )
        .await?;
    let (quoted, _item) = sales
        .add_sale_item(
            owner,
            ids,
            register.id().clone(),
            shift.id().clone(),
            sale.id(),
            AddSaleItemRequest {
                expected_sale_version: 0,
                product_id: id(PRODUCT),
                quantity: 2,
                modifier_option_ids: vec![id(OPTION)],
            },
            &at(TIME),
        )
        .await?;
    Ok((register, shift, quoted))
}

fn cash_payment() -> Vec<CheckoutPaymentRequest> {
    vec![CheckoutPaymentRequest {
        method: PaymentMethod::Cash,
        amount: "25.6912".to_owned(),
        tendered_amount: Some("30.0000".to_owned()),
    }]
}

#[tokio::test]
async fn checkout_persists_payment_consumption_and_completes_the_sale()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let mut ids = FixtureIds(1_500);
    let item_id = seed_recipe_product(&pool, &owner, &mut ids).await?;
    let (register, shift, sale) = open_sale_with_item(&pool, &owner, &mut ids).await?;

    let outcome = checkout(&pool)
        .checkout_sale(
            &owner,
            &mut ids,
            CheckoutRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
                sale_id: sale.id().clone(),
                expected_sale_version: sale.revision(),
                payments: cash_payment(),
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(outcome.sale.status(), SaleStatus::Completed);
    assert_eq!(outcome.payments.len(), 1);
    assert_eq!(outcome.payments[0].change_amount().to_string(), "4.3088");
    assert_eq!(outcome.consumption.len(), 1);
    assert_eq!(outcome.consumption[0].quantity.to_string(), "600.000000");

    let balance: String = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?",
    )
    .bind(item_id.as_str())
    .fetch_one(&pool)
    .await?;
    assert_eq!(balance, "400.000000");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT quantity_delta FROM inventory_ledger WHERE movement_type = 'SALE_CONSUMPTION' AND inventory_item_id = ?"
        )
        .bind(item_id.as_str())
        .fetch_one(&pool)
        .await?,
        "-600.000000"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sale_item_inventory_component")
            .fetch_one(&pool)
            .await?,
        1
    );
    let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM audit_event")
        .fetch_all(&pool)
        .await?;
    assert!(actions.contains(&"sale.completed".to_owned()));

    // Con la venta cobrada, el turno ya puede cerrarse.
    application(&pool)
        .close_cash_shift(
            &owner,
            &mut ids,
            register.id(),
            shift.id(),
            CloseCashShiftRequest {
                expected_version: 0,
                closing_declared_amount: "35.0000".to_owned(),
            },
            &at(LATER),
        )
        .await?;

    pool.close().await;
    let reopened = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM sale WHERE id = ?")
            .bind(sale.id().as_str())
            .fetch_one(&reopened)
            .await?,
        "COMPLETED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment")
            .fetch_one(&reopened)
            .await?,
        1
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn strict_stock_blocks_checkout_and_negative_policy_allows_it()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let mut ids = FixtureIds(1_600);
    let item_id = seed_recipe_product(&pool, &owner, &mut ids).await?;
    // Deja solo 100 g: el consumo de 600 g no cabe bajo STRICT.
    inventory(&pool)
        .adjust_stock(
            &owner,
            &mut ids,
            AdjustStockRequest {
                inventory_item_id: item_id.clone(),
                quantity_delta: "-900.000000".to_owned(),
                reason: "ajuste de prueba".to_owned(),
            },
            &at(TIME),
        )
        .await?;
    let (register, shift, sale) = open_sale_with_item(&pool, &owner, &mut ids).await?;

    assert_eq!(
        checkout(&pool)
            .checkout_sale(
                &owner,
                &mut ids,
                CheckoutRequest {
                    cash_register_id: register.id().clone(),
                    cash_shift_id: shift.id().clone(),
                    sale_id: sale.id().clone(),
                    expected_sale_version: sale.revision(),
                    payments: cash_payment(),
                },
                &at(LATER),
            )
            .await,
        Err(ApplicationError::InsufficientInventory)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment")
            .fetch_one(&pool)
            .await?,
        0,
        "el rechazo no deja pagos parciales"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM sale WHERE id = ?")
            .bind(sale.id().as_str())
            .fetch_one(&pool)
            .await?,
        "OPEN"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM inventory_ledger WHERE movement_type = 'SALE_CONSUMPTION'"
        )
        .fetch_one(&pool)
        .await?,
        0
    );

    sqlx::query("UPDATE organization SET stock_negative_policy = 'ALLOW_NEGATIVE' WHERE id = ?")
        .bind(ORG_A)
        .execute(&pool)
        .await?;
    let outcome = checkout(&pool)
        .checkout_sale(
            &owner,
            &mut ids,
            CheckoutRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
                sale_id: sale.id().clone(),
                expected_sale_version: sale.revision(),
                payments: cash_payment(),
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(outcome.sale.status(), SaleStatus::Completed);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?"
        )
        .bind(item_id.as_str())
        .fetch_one(&pool)
        .await?,
        "-500.000000"
    );
    Ok(())
}

#[tokio::test]
async fn repeated_checkout_and_payment_mismatch_do_not_duplicate_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let mut ids = FixtureIds(1_700);
    seed_recipe_product(&pool, &owner, &mut ids).await?;
    let (register, shift, sale) = open_sale_with_item(&pool, &owner, &mut ids).await?;
    let app = checkout(&pool);

    assert_eq!(
        app.checkout_sale(
            &owner,
            &mut ids,
            CheckoutRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
                sale_id: sale.id().clone(),
                expected_sale_version: sale.revision(),
                payments: vec![CheckoutPaymentRequest {
                    method: PaymentMethod::Cash,
                    amount: "25.0000".to_owned(),
                    tendered_amount: Some("25.0000".to_owned()),
                }],
            },
            &at(LATER),
        )
        .await,
        Err(ApplicationError::InvalidSaleInput),
        "la suma de pagos debe coincidir exactamente con el total"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment")
            .fetch_one(&pool)
            .await?,
        0
    );

    app.checkout_sale(
        &owner,
        &mut ids,
        CheckoutRequest {
            cash_register_id: register.id().clone(),
            cash_shift_id: shift.id().clone(),
            sale_id: sale.id().clone(),
            expected_sale_version: sale.revision(),
            payments: cash_payment(),
        },
        &at(LATER),
    )
    .await?;
    assert_eq!(
        app.checkout_sale(
            &owner,
            &mut ids,
            CheckoutRequest {
                cash_register_id: register.id().clone(),
                cash_shift_id: shift.id().clone(),
                sale_id: sale.id().clone(),
                expected_sale_version: sale.revision(),
                payments: cash_payment(),
            },
            &at(LATER),
        )
        .await,
        Err(ApplicationError::SaleNotOpen),
        "un segundo checkout no es idempotente ni duplica pagos"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment")
            .fetch_one(&pool)
            .await?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn audit_failure_rolls_back_payments_consumption_and_completion()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let mut ids = FixtureIds(1_800);
    let item_id = seed_recipe_product(&pool, &owner, &mut ids).await?;
    let (register, shift, sale) = open_sale_with_item(&pool, &owner, &mut ids).await?;

    sqlx::query("DROP TABLE audit_event").execute(&pool).await?;
    assert_eq!(
        checkout(&pool)
            .checkout_sale(
                &owner,
                &mut ids,
                CheckoutRequest {
                    cash_register_id: register.id().clone(),
                    cash_shift_id: shift.id().clone(),
                    sale_id: sale.id().clone(),
                    expected_sale_version: sale.revision(),
                    payments: cash_payment(),
                },
                &at(LATER),
            )
            .await,
        Err(ApplicationError::Audit)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM sale WHERE id = ?")
            .bind(sale.id().as_str())
            .fetch_one(&pool)
            .await?,
        "OPEN",
        "sin auditoría no hay transición a COMPLETED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sale_item_inventory_component")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM inventory_ledger WHERE movement_type = 'SALE_CONSUMPTION'"
        )
        .fetch_one(&pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?"
        )
        .bind(item_id.as_str())
        .fetch_one(&pool)
        .await?,
        "1000.000000"
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_checkout_and_shift_close_keep_a_consistent_terminal_state()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let mut ids = FixtureIds(1_900);
    seed_recipe_product(&pool, &owner, &mut ids).await?;
    let (register, shift, sale) = open_sale_with_item(&pool, &owner, &mut ids).await?;

    let second = open(
        &temp.path().join("cash.sqlite"),
        Duration::from_millis(2_000),
    )
    .await?;
    let checkout_app = checkout(&pool);
    let close_app = SqliteCashApplication::new(
        SqliteUnitOfWork::new(second.clone()),
        SqliteOpenSalesForCashShift,
    );
    let checkout_owner = owner.clone();
    let close_owner = owner.clone();

    let sale_id = sale.id().clone();
    let sale_version = sale.revision();
    let register_id = register.id().clone();
    let shift_id = shift.id().clone();
    let close_register = register.id().clone();
    let close_shift = shift.id().clone();
    let checkout_sale_id = sale_id.clone();
    let checkout_task = tokio::spawn(async move {
        let mut ids = FixtureIds(2_000);
        checkout_app
            .checkout_sale(
                &checkout_owner,
                &mut ids,
                CheckoutRequest {
                    cash_register_id: register_id,
                    cash_shift_id: shift_id,
                    sale_id: checkout_sale_id,
                    expected_sale_version: sale_version,
                    payments: cash_payment(),
                },
                &at(LATER),
            )
            .await
    });
    let close_task = tokio::spawn(async move {
        let mut ids = FixtureIds(2_100);
        close_app
            .close_cash_shift(
                &close_owner,
                &mut ids,
                &close_register,
                &close_shift,
                CloseCashShiftRequest {
                    expected_version: 0,
                    closing_declared_amount: "35.0000".to_owned(),
                },
                &at(LATER),
            )
            .await
    });

    let checkout_result = checkout_task.await?;
    let close_result = close_task.await?;
    assert!(
        checkout_result.is_ok(),
        "el checkout debe confirmarse: {checkout_result:?}"
    );
    match close_result {
        Ok(closed) => assert_eq!(closed.status().as_str(), "CLOSED"),
        Err(error) => assert_eq!(
            error,
            ApplicationError::CashShiftHasOpenSales,
            "si el cierre gana antes, la venta OPEN lo bloquea"
        ),
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM sale WHERE id = ?")
            .bind(sale_id.as_str())
            .fetch_one(&pool)
            .await?,
        "COMPLETED"
    );
    second.close().await;
    Ok(())
}
