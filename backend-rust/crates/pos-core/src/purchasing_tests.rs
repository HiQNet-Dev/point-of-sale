//! Pruebas de las reglas puras de compras.
#![allow(clippy::panic)]

use crate::{
    Currency,
    catalog::{CatalogCode, UnitPrice},
    identity::UtcSeconds,
    inventory::{InventoryBaseUnit, Quantity},
    organization_branch::{DisplayName, EntityId},
    purchasing::{
        NewPurchaseOrder, NewPurchaseOrderLine, OverReceiptViolation, PurchaseOrder,
        PurchaseOrderLine, PurchaseOrderStatus, PurchasingError, ReceiptLineRequest,
        ensure_within_ordered, next_order_status, over_receipt_violations,
        purchase_order_changed_fields, validate_receipt_lines,
    },
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn org() -> EntityId {
    id("00000000-0000-0000-0000-000000000501")
}
fn branch() -> EntityId {
    id("00000000-0000-0000-0000-000000000502")
}
fn supplier() -> EntityId {
    id("00000000-0000-0000-0000-000000000503")
}
fn other_supplier() -> EntityId {
    id("00000000-0000-0000-0000-000000000504")
}
fn creator() -> EntityId {
    id("00000000-0000-0000-0000-000000000505")
}
fn name(value: &str) -> DisplayName {
    DisplayName::new(value).unwrap_or_else(|error| panic!("fixture nombre: {error}"))
}
fn code(value: &str) -> CatalogCode {
    CatalogCode::new(value).unwrap_or_else(|error| panic!("fixture código: {error}"))
}
fn qty(value: &str) -> Quantity {
    Quantity::parse(value).unwrap_or_else(|error| panic!("fixture cantidad: {error}"))
}
fn amount(value: &str) -> UnitPrice {
    UnitPrice::parse(value).unwrap_or_else(|error| panic!("fixture importe: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture instante: {error}"))
}
fn order() -> PurchaseOrder {
    PurchaseOrder::create(NewPurchaseOrder {
        id: id("00000000-0000-0000-0000-000000000510"),
        organization_id: org(),
        branch_id: branch(),
        supplier_id: supplier(),
        code: code("oc-2026-01"),
        currency: Currency::new("MXN").unwrap_or_else(|error| panic!("moneda: {error}")),
        supplier_code_snapshot: code("proveedor-1"),
        supplier_name_snapshot: name("Proveedor Uno"),
        created_by_user_id: creator(),
    })
}
fn line(number: u32, quantity: &str) -> PurchaseOrderLine {
    PurchaseOrderLine::create(NewPurchaseOrderLine {
        id: id(&format!("00000000-0000-0000-0000-00000000052{number}")),
        organization_id: org(),
        purchase_order_id: order().id().clone(),
        inventory_item_id: id(&format!("00000000-0000-0000-0000-00000000053{number}")),
        line_number: number,
        item_code_snapshot: code("leche-entera"),
        item_name_snapshot: name("Leche entera"),
        unit_snapshot: InventoryBaseUnit::Milliliter,
        quantity: qty(quantity),
        unit_cost: Some(amount("0.02")),
    })
    .unwrap_or_else(|error| panic!("fixture partida: {error}"))
}

#[test]
fn order_is_created_in_draft_with_immutable_snapshots() {
    let order = order();
    assert_eq!(order.status(), PurchaseOrderStatus::Draft);
    assert!(order.status().is_draft());
    assert_eq!(order.currency().as_str(), "MXN");
    assert_eq!(order.supplier_code_snapshot().as_str(), "proveedor-1");
    assert_eq!(order.supplier_name_snapshot().as_str(), "Proveedor Uno");
    assert_eq!(order.revision(), 0);
    assert_eq!(order.issued_at(), None);
    assert_eq!(order.cancelled_at(), None);
    assert_eq!(order.require_draft(), Ok(()));
}

#[test]
fn issuing_is_irreversible_and_records_the_actor() {
    let mut order = order();
    let before = order.clone();
    assert_eq!(order.issue(creator(), moment(1_800_000_000)), Ok(()));
    assert_eq!(order.status(), PurchaseOrderStatus::Ordered);
    assert_eq!(order.issued_by_user_id(), Some(&creator()));
    assert_eq!(order.issued_at(), Some(moment(1_800_000_000)));
    assert_eq!(order.revision(), 1);
    assert_eq!(
        purchase_order_changed_fields(&before, &order),
        vec!["status"]
    );

    assert_eq!(
        order.issue(creator(), moment(1_800_000_100)),
        Err(PurchasingError::OrderNotDraft),
        "una orden emitida no se vuelve a emitir"
    );
    assert_eq!(order.revision(), 1);
    assert_eq!(order.require_draft(), Err(PurchasingError::OrderNotDraft));
}

#[test]
fn cancelling_is_final_and_preserves_issue_history() {
    let mut draft = order();
    assert_eq!(draft.cancel(creator(), moment(1_800_000_000)), Ok(()));
    assert_eq!(draft.status(), PurchaseOrderStatus::Cancelled);
    assert_eq!(draft.issued_at(), None, "un borrador no tiene emisión");
    assert_eq!(draft.cancelled_by_user_id(), Some(&creator()));
    assert_eq!(
        draft.cancel(creator(), moment(1_800_000_100)),
        Err(PurchasingError::OrderAlreadyCancelled)
    );

    let mut issued = order();
    issued
        .issue(creator(), moment(1_800_000_000))
        .unwrap_or_else(|error| panic!("emitir: {error}"));
    assert_eq!(issued.cancel(creator(), moment(1_800_000_200)), Ok(()));
    assert_eq!(
        issued.issued_at(),
        Some(moment(1_800_000_000)),
        "la cancelación conserva la emisión"
    );
    assert_eq!(issued.status(), PurchaseOrderStatus::Cancelled);
    assert_eq!(
        issued.issue(creator(), moment(1_800_000_300)),
        Err(PurchasingError::OrderNotDraft),
        "una orden cancelada no se emite"
    );
    assert_eq!(
        issued.apply_receipt(PurchaseOrderStatus::Received),
        Err(PurchasingError::OrderAlreadyCancelled)
    );
}

#[test]
fn supplier_change_is_draft_only_and_skips_identical_supplier() {
    let mut order = order();
    assert_eq!(
        order.change_supplier(0, supplier(), code("proveedor-1"), name("Proveedor Uno"),),
        Ok(false),
        "el mismo proveedor no genera cambio"
    );
    assert_eq!(order.revision(), 0);

    assert_eq!(
        order.change_supplier(
            0,
            other_supplier(),
            code("proveedor-2"),
            name("Proveedor Dos"),
        ),
        Ok(true)
    );
    assert_eq!(order.supplier_id(), &other_supplier());
    assert_eq!(order.supplier_code_snapshot().as_str(), "proveedor-2");
    assert_eq!(order.revision(), 1);

    order
        .issue(creator(), moment(1_800_000_000))
        .unwrap_or_else(|error| panic!("emitir: {error}"));
    assert_eq!(
        order.change_supplier(0, supplier(), code("proveedor-1"), name("Proveedor Uno"),),
        Err(PurchasingError::OrderNotDraft)
    );
}

#[test]
fn receipt_transitions_follow_the_canonical_cycle() {
    let mut order = order();
    assert_eq!(
        order.apply_receipt(PurchaseOrderStatus::PartiallyReceived),
        Err(PurchasingError::OrderNotIssued),
        "un borrador no recibe mercancía"
    );
    assert_eq!(
        order.ensure_receivable(),
        Err(PurchasingError::OrderNotIssued)
    );

    order
        .issue(creator(), moment(1_800_000_000))
        .unwrap_or_else(|error| panic!("emitir: {error}"));
    assert_eq!(order.ensure_receivable(), Ok(()));
    assert_eq!(
        order.apply_receipt(PurchaseOrderStatus::Draft),
        Err(PurchasingError::InvalidReceiptStatus),
        "una recepción no puede volver a DRAFT"
    );
    assert_eq!(
        order.apply_receipt(PurchaseOrderStatus::PartiallyReceived),
        Ok(())
    );
    assert_eq!(order.status(), PurchaseOrderStatus::PartiallyReceived);
    assert_eq!(order.ensure_receivable(), Ok(()));
    assert_eq!(order.apply_receipt(PurchaseOrderStatus::Received), Ok(()));
    assert_eq!(
        order.ensure_receivable(),
        Err(PurchasingError::OrderAlreadyReceived)
    );
    assert_eq!(order.revision(), 3);
}

#[test]
fn lines_validate_number_and_quantity() {
    assert_eq!(line(1, "10").quantity(), qty("10"));
    assert_eq!(
        PurchaseOrderLine::create(NewPurchaseOrderLine {
            id: id("00000000-0000-0000-0000-000000000540"),
            organization_id: org(),
            purchase_order_id: order().id().clone(),
            inventory_item_id: id("00000000-0000-0000-0000-000000000541"),
            line_number: 0,
            item_code_snapshot: code("leche-entera"),
            item_name_snapshot: name("Leche entera"),
            unit_snapshot: InventoryBaseUnit::Milliliter,
            quantity: qty("1"),
            unit_cost: None,
        }),
        Err(PurchasingError::InvalidLineNumber)
    );
    assert_eq!(
        PurchaseOrderLine::create(NewPurchaseOrderLine {
            id: id("00000000-0000-0000-0000-000000000542"),
            organization_id: org(),
            purchase_order_id: order().id().clone(),
            inventory_item_id: id("00000000-0000-0000-0000-000000000543"),
            line_number: 2,
            item_code_snapshot: code("leche-entera"),
            item_name_snapshot: name("Leche entera"),
            unit_snapshot: InventoryBaseUnit::Milliliter,
            quantity: qty("0"),
            unit_cost: None,
        }),
        Err(PurchasingError::InvalidQuantity)
    );
    assert_eq!(
        line(1, "1").ensure_unit_unchanged(InventoryBaseUnit::Milliliter),
        Ok(())
    );
    assert_eq!(
        line(1, "1").ensure_unit_unchanged(InventoryBaseUnit::Gram),
        Err(PurchasingError::UnitChanged)
    );
}

#[test]
fn agreed_cost_distinguishes_unknown_from_zero() {
    // ADR-0021 CC21-03: el costo pactado es opcional. `None` significa "no
    // informado" y nunca debe confundirse con un costo cero, que es un valor
    // explícito del operador.
    let unknown = NewPurchaseOrderLine {
        id: id("00000000-0000-0000-0000-0000000005a1"),
        organization_id: org(),
        purchase_order_id: order().id().clone(),
        inventory_item_id: id("00000000-0000-0000-0000-0000000005a2"),
        line_number: 1,
        item_code_snapshot: code("leche-entera"),
        item_name_snapshot: name("Leche entera"),
        unit_snapshot: InventoryBaseUnit::Milliliter,
        quantity: qty("1"),
        unit_cost: None,
    };
    let created = PurchaseOrderLine::create(unknown);
    assert!(created.is_ok(), "una partida sin costo pactado es válida");
    let Ok(created) = created else { return };
    assert_eq!(created.unit_cost(), None);

    let zero = PurchaseOrderLine::restore(
        id("00000000-0000-0000-0000-0000000005a3"),
        org(),
        order().id().clone(),
        id("00000000-0000-0000-0000-0000000005a4"),
        1,
        code("leche-entera"),
        name("Leche entera"),
        InventoryBaseUnit::Milliliter,
        qty("1"),
        Some(amount("0.0000")),
    );
    assert!(zero.is_ok(), "un costo cero explícito es representable");
    let Ok(zero) = zero else { return };
    assert_eq!(zero.unit_cost(), Some(amount("0.0000")));
    assert_ne!(
        zero.unit_cost(),
        created.unit_cost(),
        "desconocido y cero no son el mismo hecho económico"
    );
}

#[test]
fn receipt_lines_reject_duplicates_and_non_positive_quantities() {
    let repeated = vec![
        ReceiptLineRequest {
            purchase_order_line_id: line(1, "10").id().clone(),
            quantity: qty("1"),
        },
        ReceiptLineRequest {
            purchase_order_line_id: line(1, "10").id().clone(),
            quantity: qty("2"),
        },
    ];
    assert_eq!(
        validate_receipt_lines(&repeated),
        Err(PurchasingError::DuplicateReceiptLine)
    );
    let zero = vec![ReceiptLineRequest {
        purchase_order_line_id: line(1, "10").id().clone(),
        quantity: qty("0"),
    }];
    assert_eq!(
        validate_receipt_lines(&zero),
        Err(PurchasingError::InvalidQuantity)
    );
    let ok = vec![ReceiptLineRequest {
        purchase_order_line_id: line(1, "10").id().clone(),
        quantity: qty("1"),
    }];
    assert_eq!(validate_receipt_lines(&ok), Ok(()));
}

#[test]
fn over_receipt_is_rejected_and_reports_the_violation() {
    let first = line(1, "10");
    let second = line(2, "5");
    let lines = vec![first.clone(), second.clone()];
    let already = vec![(first.id().clone(), qty("8"))];
    let requested = vec![
        ReceiptLineRequest {
            purchase_order_line_id: first.id().clone(),
            quantity: qty("3"),
        },
        ReceiptLineRequest {
            purchase_order_line_id: second.id().clone(),
            quantity: qty("5"),
        },
    ];

    assert_eq!(
        ensure_within_ordered(&lines, &already, &requested),
        Err(PurchasingError::ReceiptExceedsOrdered)
    );
    assert_eq!(
        over_receipt_violations(&lines, &already, &requested),
        vec![OverReceiptViolation {
            line_number: 1,
            ordered: qty("10"),
            already_received: qty("8"),
            attempted: qty("3"),
        }]
    );

    let exact = vec![ReceiptLineRequest {
        purchase_order_line_id: first.id().clone(),
        quantity: qty("2"),
    }];
    assert_eq!(ensure_within_ordered(&lines, &already, &exact), Ok(()));
}

#[test]
fn next_status_reports_partial_then_received() {
    let first = line(1, "10");
    let second = line(2, "5");
    let lines = vec![first.clone(), second.clone()];

    let partial = vec![ReceiptLineRequest {
        purchase_order_line_id: first.id().clone(),
        quantity: qty("10"),
    }];
    assert_eq!(
        next_order_status(&lines, &[], &partial),
        PurchaseOrderStatus::PartiallyReceived
    );

    let full = vec![
        ReceiptLineRequest {
            purchase_order_line_id: first.id().clone(),
            quantity: qty("10"),
        },
        ReceiptLineRequest {
            purchase_order_line_id: second.id().clone(),
            quantity: qty("5"),
        },
    ];
    assert_eq!(
        next_order_status(&lines, &[], &full),
        PurchaseOrderStatus::Received
    );

    let accumulated = vec![
        (first.id().clone(), qty("4")),
        (second.id().clone(), qty("5")),
    ];
    let rest = vec![ReceiptLineRequest {
        purchase_order_line_id: first.id().clone(),
        quantity: qty("6"),
    }];
    assert_eq!(
        next_order_status(&lines, &accumulated, &rest),
        PurchaseOrderStatus::Received,
        "las recepciones previas cuentan para el estado final"
    );
}

#[test]
fn restored_orders_and_statuses_reject_incoherent_data() {
    assert_eq!(
        PurchaseOrderStatus::parse("PARTIALLY_RECEIVED"),
        Ok(PurchaseOrderStatus::PartiallyReceived)
    );
    assert_eq!(
        PurchaseOrderStatus::parse("REOPENED"),
        Err(PurchasingError::InvalidPersistedValue)
    );
    assert!(PurchaseOrderStatus::Ordered.is_receivable());
    assert!(!PurchaseOrderStatus::Draft.is_receivable());

    assert_eq!(
        PurchaseOrder::restore(
            id("00000000-0000-0000-0000-000000000550"),
            org(),
            branch(),
            supplier(),
            code("oc-2026-01"),
            PurchaseOrderStatus::Ordered,
            Currency::new("MXN").unwrap_or_else(|error| panic!("moneda: {error}")),
            code("proveedor-1"),
            name("Proveedor Uno"),
            creator(),
            None,
            None,
            None,
            None,
            1,
        ),
        Err(PurchasingError::InvalidPersistedValue),
        "ORDERED exige emisión"
    );
    assert_eq!(
        PurchaseOrderLine::restore(
            id("00000000-0000-0000-0000-000000000551"),
            org(),
            order().id().clone(),
            id("00000000-0000-0000-0000-000000000552"),
            1,
            code("leche-entera"),
            name("Leche entera"),
            InventoryBaseUnit::Milliliter,
            qty("0"),
            None,
        ),
        Err(PurchasingError::InvalidPersistedValue),
        "una partida no puede tener cantidad cero"
    );
}
