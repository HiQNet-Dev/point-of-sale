//! Pruebas de adaptación con el fixture existente; no son E2E productivos.
use super::{PurchasingState, commands::*, dto::*};
use crate::dto::IpcError;
use pos_core::{
    audit::AuditOccurredAt,
    identity::{SessionHash, UtcSeconds},
    organization_branch::{ApplicationError, EntityId},
};
use std::{
    error::Error,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tauri::Manager;

static NEXT: AtomicU64 = AtomicU64::new(0x2c00_0000);
fn next_id() -> Result<EntityId, ApplicationError> {
    EntityId::parse(&format!(
        "00000000-0000-0000-0000-{:012x}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
    .map_err(|_| ApplicationError::Storage)
}
fn clock() -> Result<(UtcSeconds, AuditOccurredAt), IpcError> {
    Ok((
        UtcSeconds::new(1_793_350_800).map_err(|_| IpcError::InternalError)?,
        AuditOccurredAt::new(pos_lab::LAB_OCCURRED_AT).map_err(|_| IpcError::InternalError)?,
    ))
}
fn input<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_value(value)?)
}

#[test]
fn supplier_handlers_use_application_and_revalidate_permissions_and_session()
-> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let pool = pos_storage_sqlite::open(
            &directory.path().join("existing-fixture.sqlite"),
            Duration::from_secs(5),
        )
        .await?;
        pos_lab::run(&pool).await?;
        let hash: String = sqlx::query_scalar("SELECT session_hash FROM local_session LIMIT 1")
            .fetch_one(&pool)
            .await?;
        let state =
            PurchasingState::candidate(pool.clone(), SessionHash::new(&hash)?, clock, next_id);
        let app = tauri::test::mock_builder()
            .manage(state)
            .build(tauri::generate_context!())?;
        let created = purchasing_create_supplier(app.state(), input(serde_json::json!({"code":"supplier-2c","name":"Proveedor 2C","email":"purchase@hiqnet.test"}))?).await?;
        assert_eq!(created.code, "supplier-2c");
        assert_eq!(
            purchasing_get_supplier(
                app.state(),
                SupplierKey {
                    supplier_id: created.id.clone()
                }
            )
            .await?
            .name,
            "Proveedor 2C"
        );
        assert_eq!(
            purchasing_list_suppliers(
                app.state(),
                SupplierList {
                    limit: 20,
                    offset: 0,
                    status: None
                }
            )
            .await?
            .len(),
            1
        );
        let updated = purchasing_update_supplier(app.state(), input(serde_json::json!({"supplierId":created.id,"expectedVersion":created.version,"name":"Actualizado"}))?).await?;
        assert_eq!(updated.name, "Actualizado");
        let stale = purchasing_update_supplier(app.state(), input(serde_json::json!({"supplierId":created.id,"expectedVersion":created.version,"name":"Obsoleto"}))?).await;
        assert_eq!(stale.err(), Some(IpcError::Conflict));
        sqlx::query(
            "DELETE FROM role_permission WHERE permission_code = 'purchasing.supplier.read'",
        )
        .execute(&pool)
        .await?;
        assert_eq!(
            purchasing_get_supplier(
                app.state(),
                SupplierKey {
                    supplier_id: created.id.clone()
                }
            )
            .await
            .err(),
            Some(IpcError::Forbidden)
        );
        sqlx::query(
            "UPDATE local_session SET state = 'REVOKED', revoked_at = '2026-10-30T10:00:00Z'",
        )
        .execute(&pool)
        .await?;
        assert_eq!(
            purchasing_get_supplier(
                app.state(),
                SupplierKey {
                    supplier_id: created.id
                }
            )
            .await
            .err(),
            Some(IpcError::Unauthorized)
        );
        pool.close().await;
        Ok(())
    })
}

#[test]
fn order_and_receipt_handlers_delegate_the_complete_candidate_flow() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let pool = pos_storage_sqlite::open(
            &directory.path().join("existing-fixture.sqlite"),
            Duration::from_secs(5),
        )
        .await?;
        let fixture = pos_lab::run(&pool).await?;
        let hash: String = sqlx::query_scalar("SELECT session_hash FROM local_session LIMIT 1")
            .fetch_one(&pool)
            .await?;
        let state =
            PurchasingState::candidate(pool.clone(), SessionHash::new(&hash)?, clock, next_id);
        let app = tauri::test::mock_builder()
            .manage(state)
            .build(tauri::generate_context!())?;
        let supplier = purchasing_create_supplier(
            app.state(),
            input(serde_json::json!({"code":"supplier-orders","name":"Proveedor"}))?,
        )
        .await?;
        let second_supplier = purchasing_create_supplier(
            app.state(),
            input(serde_json::json!({"code":"supplier-other","name":"Otro"}))?,
        )
        .await?;
        let branch = fixture["branch_id"].as_str().ok_or("falta branch")?;
        let item = fixture["inventory_item_id"].as_str().ok_or("falta item")?;
        let order = purchasing_create_order(app.state(), input(serde_json::json!({"branchId":branch,"code":"oc-2c","supplierId":supplier.id,"lines":[{"inventoryItemId":item,"quantity":"2.000000","unitCost":null}]}))?).await?;
        assert!(order.total.is_none());
        let key = || OrderKey {
            branch_id: branch.into(),
            order_id: order.id.clone(),
        };
        assert_eq!(purchasing_get_order(app.state(), key()).await?.id, order.id);
        assert_eq!(
            purchasing_list_orders(
                app.state(),
                OrderList {
                    branch_id: branch.into(),
                    limit: 20,
                    offset: 0,
                    status: None
                }
            )
            .await?
            .len(),
            1
        );
        let changed = purchasing_change_supplier(
            app.state(),
            SupplierChange {
                branch_id: branch.into(),
                order_id: order.id.clone(),
                supplier_id: second_supplier.id,
                expected_version: order.version.clone(),
            },
        )
        .await?;
        let updated = purchasing_update_line(
            app.state(),
            LineUpdate {
                branch_id: branch.into(),
                order_id: order.id.clone(),
                line_id: changed.lines[0].id.clone(),
                expected_version: changed.version,
                quantity: "2.000000".into(),
                unit_cost: TextPatch::Set("0.0000".into()),
            },
        )
        .await?;
        assert_eq!(updated.total.as_deref(), Some("0.0000000000"));
        let removed = purchasing_remove_line(
            app.state(),
            LineRemove {
                branch_id: branch.into(),
                order_id: order.id.clone(),
                line_id: updated.lines[0].id.clone(),
                expected_version: updated.version,
            },
        )
        .await?;
        assert!(removed.lines.is_empty());
        let added = purchasing_add_line(
            app.state(),
            LineAdd {
                branch_id: branch.into(),
                order_id: order.id.clone(),
                line: LineInput {
                    inventory_item_id: item.into(),
                    quantity: "2.000000".into(),
                    unit_cost: None,
                },
            },
        )
        .await?;
        assert!(added.total.is_none());
        let issued = purchasing_issue_order(
            app.state(),
            OrderTransition {
                branch_id: branch.into(),
                order_id: order.id.clone(),
                expected_version: added.version,
            },
        )
        .await?;
        assert_eq!(issued.status, "ORDERED");
        let receipt_request = || ReceiptInput {
            branch_id: branch.into(),
            order_id: order.id.clone(),
            expected_version: issued.version.clone(),
            idempotency_key: "receipt-2c".into(),
            lines: vec![ReceiptLineInput {
                purchase_order_line_id: issued.lines[0].id.clone(),
                quantity: "0.500000".into(),
            }],
        };
        let posted = purchasing_post_receipt(app.state(), receipt_request()).await?;
        assert_eq!(
            purchasing_post_receipt(app.state(), receipt_request())
                .await?
                .id,
            posted.id
        );
        let mut wrong_scope = receipt_request();
        wrong_scope.branch_id = "00000000-0000-0000-0000-000000002c99".into();
        assert_eq!(
            purchasing_post_receipt(app.state(), wrong_scope)
                .await
                .err(),
            Some(IpcError::Forbidden)
        );
        let mut changed_payload = receipt_request();
        changed_payload.lines[0].quantity = "0.250000".into();
        assert_eq!(
            purchasing_post_receipt(app.state(), changed_payload)
                .await
                .err(),
            Some(IpcError::Conflict)
        );
        assert_eq!(
            purchasing_get_receipt(
                app.state(),
                ReceiptKey {
                    branch_id: branch.into(),
                    receipt_id: posted.id.clone()
                }
            )
            .await?
            .lines[0]
                .quantity,
            "0.500000"
        );
        assert_eq!(
            purchasing_list_receipts(
                app.state(),
                ReceiptList {
                    branch_id: branch.into(),
                    order_id: Some(order.id.clone()),
                    limit: 20,
                    offset: 0
                }
            )
            .await?
            .len(),
            1
        );
        let status = purchasing_receipt_status(app.state(), key()).await?;
        assert_eq!(status[0].pending_quantity, "1.500000");
        let current = purchasing_get_order(app.state(), key()).await?;
        let mut over = receipt_request();
        over.expected_version = current.version;
        over.idempotency_key = "over-receipt-2c".into();
        over.lines[0].quantity = "3.000000".into();
        assert_eq!(
            purchasing_post_receipt(app.state(), over).await.err(),
            Some(IpcError::ValidationError)
        );
        let other_branch = purchasing_get_order(
            app.state(),
            OrderKey {
                branch_id: "00000000-0000-0000-0000-000000002c99".into(),
                order_id: order.id.clone(),
            },
        )
        .await;
        assert!(matches!(
            other_branch.err(),
            Some(IpcError::NotFound | IpcError::Forbidden)
        ));
        let cancel = purchasing_create_order(app.state(), input(serde_json::json!({"branchId":branch,"code":"oc-cancel-2c","supplierId":supplier.id,"lines":[]}))?).await?;
        assert_eq!(
            purchasing_cancel_order(
                app.state(),
                OrderTransition {
                    branch_id: branch.into(),
                    order_id: cancel.id,
                    expected_version: cancel.version
                }
            )
            .await?
            .status,
            "CANCELLED"
        );
        let audits: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM audit_event WHERE action LIKE 'purchase%'")
                .fetch_one(&pool)
                .await?;
        assert!(audits > 0);
        pool.close().await;
        Ok(())
    })
}

#[test]
fn transport_preserves_patch_semantics_and_exact_versions() -> Result<(), Box<dyn Error>> {
    assert!(TextPatch::Keep.nested().is_none());
    assert_eq!(TextPatch::Clear.nested(), Some(None));
    assert_eq!(
        TextPatch::Set("0.0000".into()).nested(),
        Some(Some("0.0000".into()))
    );
    assert_eq!(version("9007199254740993")?, 9_007_199_254_740_993);
    for invalid in ["-1", "01", "1.0", "18446744073709551616"] {
        assert_eq!(version(invalid), Err(IpcError::ValidationError));
    }
    assert!(
        input::<SupplierInput>(serde_json::json!({"code":"x","name":"x","actorUserId":"forged"}))
            .is_err()
    );
    Ok(())
}

#[test]
fn purchasing_bindings_match_and_runtime_stays_closed() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("purchasing.ts");
    crate::bindings::export_purchasing(&path)?;
    let generated = std::fs::read_to_string(path)?;
    let checked_in = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../frontend/src/shared/rust-local/purchasing.generated.ts"),
    )?;
    assert_eq!(
        generated.replace("\r\n", "\n"),
        checked_in.replace("\r\n", "\n")
    );
    assert_eq!(
        generated.matches("__TAURI_INVOKE(\"purchasing_").count(),
        17
    );
    assert!(
        !crate::command_names::COMMAND_NAMES
            .iter()
            .any(|name| name.starts_with("purchasing_"))
    );
    let documentation = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/03-api/rust-purchasing-ipc.md"),
    )?;
    for line in generated
        .lines()
        .filter(|line| line.contains("__TAURI_INVOKE(\"purchasing_"))
    {
        let command = line
            .split("__TAURI_INVOKE(\"")
            .nth(1)
            .and_then(|tail| tail.split('"').next())
            .ok_or("comando generado inválido")?;
        assert!(
            documentation.contains(&format!("`{command}`")),
            "comando sin documentar: {command}"
        );
    }
    let source = include_str!("commands.rs");
    for forbidden in ["sqlx", "pos_storage", "pos_lab", "BEGIN", "COMMIT"] {
        assert!(!source.contains(forbidden));
    }
    Ok(())
}
