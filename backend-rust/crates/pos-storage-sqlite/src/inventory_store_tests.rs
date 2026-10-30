//! Integración SQLite real de inventario candidato: artículos, saldos y ledger.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_core::{
    identity::VerifiedPrincipal,
    inventory::{InventoryBaseUnit, InventoryMovementType},
    organization_branch::ApplicationError,
};

use super::*;
use crate::{
    cash_store_tests::{
        BRANCH_A, BRANCH_OTHER, FixtureIds, HASH_A, HASH_B, LATER, TIME, at, id, open_candidate,
        principal,
    },
    inventory::{
        AdjustStockRequest, InventoryPage, NewInventoryItemRequest, OpenStockRequest,
        SqliteInventoryApplication,
    },
};

fn inventory(pool: &SqlitePool) -> SqliteInventoryApplication {
    SqliteInventoryApplication::new(SqliteUnitOfWork::new(pool.clone()))
}

async fn create_item(
    app: &SqliteInventoryApplication,
    owner: &VerifiedPrincipal,
    ids: &mut FixtureIds,
    code: &str,
    base_unit: InventoryBaseUnit,
    fractional_unit_allowed: bool,
) -> Result<pos_core::inventory::InventoryItem, ApplicationError> {
    app.create_item(
        owner,
        ids,
        NewInventoryItemRequest {
            code: code.to_owned(),
            name: "Harina".to_owned(),
            description: Some("Insumo de cocina".to_owned()),
            base_unit,
            fractional_unit_allowed,
        },
        &at(TIME),
    )
    .await
}

#[tokio::test]
async fn open_adjust_and_reopen_preserve_balance_and_append_only_ledger()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let inventory = inventory(&pool);
    let mut ids = FixtureIds(700);
    let item = create_item(
        &inventory,
        &owner,
        &mut ids,
        "harina",
        InventoryBaseUnit::Gram,
        true,
    )
    .await?;

    let (balance, opening) = inventory
        .open_stock(
            &owner,
            &mut ids,
            OpenStockRequest {
                inventory_item_id: item.id().clone(),
                quantity: "1000.000000".to_owned(),
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(balance.quantity_on_hand().to_string(), "1000.000000");
    assert_eq!(opening.movement_type(), InventoryMovementType::Opening);

    let (adjusted, movement) = inventory
        .adjust_stock(
            &owner,
            &mut ids,
            AdjustStockRequest {
                inventory_item_id: item.id().clone(),
                quantity_delta: "-250.500000".to_owned(),
                reason: "merma de bodega".to_owned(),
            },
            &at(LATER),
        )
        .await?;
    assert_eq!(adjusted.quantity_on_hand().to_string(), "749.500000");
    assert_eq!(movement.movement_type(), InventoryMovementType::Adjustment);

    let current = inventory
        .get_balance(&owner, item.id())
        .await?
        .ok_or("saldo ausente")?;
    assert_eq!(current.quantity_on_hand().to_string(), "749.500000");
    assert_eq!(current.revision(), 2);

    let movements = inventory
        .list_movements(
            &owner,
            item.id(),
            InventoryPage {
                limit: 10,
                offset: 0,
            },
        )
        .await?;
    assert_eq!(movements.len(), 2);

    let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM audit_event ORDER BY action")
        .fetch_all(&pool)
        .await?;
    assert!(actions.contains(&"inventory.item.created".to_owned()));
    assert!(actions.contains(&"inventory.stock.opened".to_owned()));
    assert!(actions.contains(&"inventory.stock.adjusted".to_owned()));

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT unit_locked FROM inventory_item WHERE id = ?")
            .bind(item.id().as_str())
            .fetch_one(&pool)
            .await?,
        1,
        "el primer movimiento congela la unidad base"
    );

    // El ledger es append-only también frente a SQL directo.
    assert!(
        sqlx::query("UPDATE inventory_ledger SET quantity_delta = '1.000000'")
            .execute(&pool)
            .await
            .is_err(),
        "el trigger debe rechazar la reescritura del historial"
    );
    assert!(
        sqlx::query("DELETE FROM inventory_ledger")
            .execute(&pool)
            .await
            .is_err(),
        "el trigger debe rechazar el borrado del historial"
    );

    pool.close().await;
    let reopened = open(&temp.path().join("cash.sqlite"), Duration::from_millis(500)).await?;
    let persisted: String = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?",
    )
    .bind(item.id().as_str())
    .fetch_one(&reopened)
    .await?;
    assert_eq!(persisted, "749.500000");
    let ledger: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_ledger")
        .fetch_one(&reopened)
        .await?;
    assert_eq!(ledger, 2);
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn duplicate_code_fractional_unit_and_opening_rules_are_enforced()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let outsider = principal(&pool, HASH_B, &id(BRANCH_OTHER), false).await?;
    let inventory = inventory(&pool);
    let mut ids = FixtureIds(800);

    let integer_item = create_item(
        &inventory,
        &owner,
        &mut ids,
        "unidad",
        InventoryBaseUnit::Unit,
        false,
    )
    .await?;
    assert_eq!(
        create_item(
            &inventory,
            &owner,
            &mut ids,
            " unidad ",
            InventoryBaseUnit::Unit,
            false,
        )
        .await,
        Err(ApplicationError::InventoryItemCodeAlreadyExists),
        "el código se normaliza y es único por organización"
    );
    assert_eq!(
        create_item(
            &inventory,
            &outsider,
            &mut ids,
            "ajeno",
            InventoryBaseUnit::Unit,
            false,
        )
        .await,
        Err(ApplicationError::Forbidden),
        "un principal de otra organización no crea artículos"
    );

    assert_eq!(
        inventory
            .open_stock(
                &owner,
                &mut ids,
                OpenStockRequest {
                    inventory_item_id: integer_item.id().clone(),
                    quantity: "1.500000".to_owned(),
                },
                &at(TIME),
            )
            .await,
        Err(ApplicationError::InvalidInventoryInput),
        "una unidad discreta no admite fracciones"
    );
    assert_eq!(
        inventory
            .adjust_stock(
                &owner,
                &mut ids,
                AdjustStockRequest {
                    inventory_item_id: integer_item.id().clone(),
                    quantity_delta: "1.000000".to_owned(),
                    reason: "alta manual".to_owned(),
                },
                &at(TIME),
            )
            .await,
        Err(ApplicationError::InvalidInventoryInput),
        "no se ajusta un artículo sin apertura"
    );

    inventory
        .open_stock(
            &owner,
            &mut ids,
            OpenStockRequest {
                inventory_item_id: integer_item.id().clone(),
                quantity: "5.000000".to_owned(),
            },
            &at(TIME),
        )
        .await?;
    assert_eq!(
        inventory
            .open_stock(
                &owner,
                &mut ids,
                OpenStockRequest {
                    inventory_item_id: integer_item.id().clone(),
                    quantity: "5.000000".to_owned(),
                },
                &at(TIME),
            )
            .await,
        Err(ApplicationError::InventoryOpeningAlreadyExists),
        "la apertura es única por sucursal y artículo"
    );

    assert_eq!(
        inventory
            .adjust_stock(
                &owner,
                &mut ids,
                AdjustStockRequest {
                    inventory_item_id: integer_item.id().clone(),
                    quantity_delta: "1.500000".to_owned(),
                    reason: "ajuste fraccionario".to_owned(),
                },
                &at(LATER),
            )
            .await,
        Err(ApplicationError::InvalidInventoryInput),
        "el ajuste respeta la unidad base"
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_openings_confirm_exactly_one_opening() -> Result<(), Box<dyn std::error::Error>>
{
    let (temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let inventory = inventory(&pool);
    let mut ids = FixtureIds(900);
    let item = create_item(
        &inventory,
        &owner,
        &mut ids,
        "concurrente",
        InventoryBaseUnit::Unit,
        true,
    )
    .await?;

    let second = open(
        &temp.path().join("cash.sqlite"),
        Duration::from_millis(2_000),
    )
    .await?;
    let first_app = inventory;
    let second_app = SqliteInventoryApplication::new(SqliteUnitOfWork::new(second.clone()));
    let item_id = item.id().clone();
    let first_owner = owner.clone();
    let second_owner = owner.clone();

    let first = tokio::spawn(async move {
        let mut ids = FixtureIds(1_000);
        first_app
            .open_stock(
                &first_owner,
                &mut ids,
                OpenStockRequest {
                    inventory_item_id: item_id,
                    quantity: "4.000000".to_owned(),
                },
                &at(TIME),
            )
            .await
    });
    let other_id = item.id().clone();
    let second_task = tokio::spawn(async move {
        let mut ids = FixtureIds(1_100);
        second_app
            .open_stock(
                &second_owner,
                &mut ids,
                OpenStockRequest {
                    inventory_item_id: other_id,
                    quantity: "7.000000".to_owned(),
                },
                &at(TIME),
            )
            .await
    });

    let outcomes = [first.await?, second_task.await?];
    let succeeded = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
    assert_eq!(
        succeeded, 1,
        "solo una apertura puede confirmarse: {outcomes:?}"
    );

    let openings: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_ledger WHERE inventory_item_id = ? AND movement_type = 'OPENING'",
    )
    .bind(item.id().as_str())
    .fetch_one(&pool)
    .await?;
    assert_eq!(openings, 1);
    let quantity: String = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?",
    )
    .bind(item.id().as_str())
    .fetch_one(&pool)
    .await?;
    assert!(quantity == "4.000000" || quantity == "7.000000");
    second.close().await;
    Ok(())
}

#[tokio::test]
async fn concurrent_adjustments_serialize_without_lost_updates()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let owner = principal(&pool, HASH_A, &id(BRANCH_A), false).await?;
    let inventory = inventory(&pool);
    let mut ids = FixtureIds(1_200);
    let item = create_item(
        &inventory,
        &owner,
        &mut ids,
        "lotes",
        InventoryBaseUnit::Unit,
        true,
    )
    .await?;
    inventory
        .open_stock(
            &owner,
            &mut ids,
            OpenStockRequest {
                inventory_item_id: item.id().clone(),
                quantity: "0.000000".to_owned(),
            },
            &at(TIME),
        )
        .await?;

    let second = open(
        &temp.path().join("cash.sqlite"),
        Duration::from_millis(2_000),
    )
    .await?;
    let first_app = inventory;
    let second_app = SqliteInventoryApplication::new(SqliteUnitOfWork::new(second.clone()));
    let first_owner = owner.clone();
    let second_owner = owner.clone();
    let item_id = item.id().clone();

    let first = tokio::spawn(async move {
        let mut ids = FixtureIds(1_300);
        first_app
            .adjust_stock(
                &first_owner,
                &mut ids,
                AdjustStockRequest {
                    inventory_item_id: item_id,
                    quantity_delta: "1.000000".to_owned(),
                    reason: "conteo a".to_owned(),
                },
                &at(LATER),
            )
            .await
    });
    let other_id = item.id().clone();
    let second_task = tokio::spawn(async move {
        let mut ids = FixtureIds(1_400);
        second_app
            .adjust_stock(
                &second_owner,
                &mut ids,
                AdjustStockRequest {
                    inventory_item_id: other_id,
                    quantity_delta: "2.000000".to_owned(),
                    reason: "conteo b".to_owned(),
                },
                &at(LATER),
            )
            .await
    });

    let outcomes = [first.await?, second_task.await?];
    let confirmed = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
    let quantity: String = sqlx::query_scalar(
        "SELECT quantity_on_hand FROM inventory_balance WHERE inventory_item_id = ?",
    )
    .bind(item.id().as_str())
    .fetch_one(&pool)
    .await?;
    let expected = match confirmed {
        2 => "3.000000",
        1 => {
            if outcomes[0].is_ok() {
                "1.000000"
            } else {
                "2.000000"
            }
        }
        other => return Err(format!("ninguna escritura confirmada: {other}").into()),
    };
    assert_eq!(
        quantity, expected,
        "el saldo refleja exactamente lo confirmado"
    );
    second.close().await;
    Ok(())
}
