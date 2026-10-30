//! Pruebas de las reglas puras de ventas, pagos y modificadores.
#![allow(clippy::panic)]

use crate::{
    catalog::{CatalogCode, PriceDelta, Sku, UnitPrice},
    identity::UtcSeconds,
    organization_branch::{DisplayName, EntityId},
    sales::{
        CheckoutInstruction, ModifierGroupAssignment, NewPayment, NewSale, NewSaleItem, Payment,
        PaymentMethod, Sale, SaleError, SaleItem, SaleItemStatus, SaleScope, SaleStatus,
        SelectedModifierOption, resolve_modifier_pricing, total_of_active_items, validate_checkout,
    },
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn code(value: &str) -> CatalogCode {
    CatalogCode::new(value).unwrap_or_else(|error| panic!("fixture código: {error}"))
}
fn scope() -> SaleScope {
    SaleScope {
        organization_id: id("00000000-0000-0000-0000-000000000601"),
        branch_id: id("00000000-0000-0000-0000-000000000602"),
        cash_register_id: id("00000000-0000-0000-0000-000000000603"),
        cash_shift_id: id("00000000-0000-0000-0000-000000000604"),
    }
}
fn actor() -> EntityId {
    id("00000000-0000-0000-0000-000000000605")
}
fn other_scope() -> SaleScope {
    SaleScope {
        cash_shift_id: id("00000000-0000-0000-0000-000000000606"),
        ..scope()
    }
}
fn name(value: &str) -> DisplayName {
    DisplayName::new(value).unwrap_or_else(|error| panic!("fixture nombre: {error}"))
}
fn sku(value: &str) -> Sku {
    Sku::new(value).unwrap_or_else(|error| panic!("fixture SKU: {error}"))
}
fn amount(value: &str) -> UnitPrice {
    UnitPrice::parse(value).unwrap_or_else(|error| panic!("fixture importe: {error}"))
}
fn delta(value: &str) -> PriceDelta {
    PriceDelta::parse(value).unwrap_or_else(|error| panic!("fixture delta: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture instante: {error}"))
}
fn sale() -> Sale {
    Sale::open(NewSale {
        id: id("00000000-0000-0000-0000-000000000610"),
        scope: scope(),
        created_by_user_id: actor(),
    })
}
fn item(position: u32, effective: &str, quantity: u32) -> SaleItem {
    SaleItem::create(NewSaleItem {
        id: id(&format!("00000000-0000-0000-0000-00000000062{position}")),
        organization_id: scope().organization_id,
        sale_id: sale().id().clone(),
        product_id: id(&format!("00000000-0000-0000-0000-00000000063{position}")),
        product_sku_snapshot: sku("SKU-1"),
        product_name_snapshot: name("Café americano"),
        base_unit_price: amount("35"),
        modifier_unit_delta: delta("0"),
        effective_unit_price: amount(effective),
        quantity,
        position,
    })
    .unwrap_or_else(|error| panic!("fixture línea: {error}"))
}

#[test]
fn sale_opens_with_zero_total_and_exact_scope() {
    let sale = sale();
    assert_eq!(sale.status(), SaleStatus::Open);
    assert_eq!(sale.total_amount().to_string(), "0.0000");
    assert_eq!(sale.revision(), 0);
    assert_eq!(sale.ensure_scope(&scope()), Ok(()));
    assert_eq!(
        sale.ensure_scope(&other_scope()),
        Err(SaleError::OwnershipMismatch),
        "otro turno es otro scope operativo"
    );
    assert_eq!(sale.require_open(), Ok(()));
}

#[test]
fn total_recalculation_is_atomic_and_detects_no_op() {
    let mut sale = sale();
    assert_eq!(sale.recalculate_total(amount("70")), Ok(true));
    assert_eq!(sale.total_amount().to_string(), "70.0000");
    assert_eq!(sale.revision(), 1);
    assert_eq!(
        sale.recalculate_total(amount("70")),
        Ok(false),
        "el mismo total no incrementa la revisión"
    );
    assert_eq!(sale.revision(), 1);
}

#[test]
fn sale_lifecycle_is_final_after_completion_or_cancellation() {
    let mut completed = sale();
    completed
        .complete(actor(), moment(1_800_000_000))
        .unwrap_or_else(|error| panic!("cobrar: {error}"));
    assert_eq!(completed.status(), SaleStatus::Completed);
    assert_eq!(completed.completed_by_user_id(), Some(&actor()));
    assert_eq!(completed.completed_at(), Some(moment(1_800_000_000)));
    assert_eq!(completed.require_open(), Err(SaleError::SaleNotOpen));
    assert_eq!(
        completed.cancel(actor(), moment(1_800_000_100)),
        Err(SaleError::SaleAlreadyCompleted)
    );
    assert_eq!(
        completed.recalculate_total(amount("1")),
        Err(SaleError::SaleNotOpen)
    );

    let mut cancelled = sale();
    assert_eq!(cancelled.cancel(actor(), moment(1_800_000_000)), Ok(()));
    assert_eq!(cancelled.status(), SaleStatus::Cancelled);
    assert_eq!(
        cancelled.cancel(actor(), moment(1_800_000_100)),
        Err(SaleError::SaleAlreadyCancelled)
    );
    assert_eq!(
        cancelled.complete(actor(), moment(1_800_000_200)),
        Err(SaleError::SaleNotOpen)
    );
}

#[test]
fn sale_item_computes_an_exact_line_total() {
    let line = item(0, "35", 3);
    assert_eq!(line.line_total().to_string(), "105.0000");
    assert_eq!(line.quantity(), 3);
    assert_eq!(line.product_sku_snapshot().as_str(), "SKU-1");
    assert_eq!(line.status(), SaleItemStatus::Active);
    assert!(line.is_active());
    assert_eq!(line.require_active(), Ok(()));

    let fractional = item(1, "0.0001", 3);
    assert_eq!(
        fractional.line_total().to_string(),
        "0.0003",
        "la multiplicación es exacta, sin redondeo"
    );
}

#[test]
fn sale_item_rejects_invalid_quantities_and_overflow() {
    assert_eq!(
        SaleItem::create(NewSaleItem {
            id: id("00000000-0000-0000-0000-000000000640"),
            organization_id: scope().organization_id,
            sale_id: sale().id().clone(),
            product_id: id("00000000-0000-0000-0000-000000000641"),
            product_sku_snapshot: sku("SKU-1"),
            product_name_snapshot: name("Café"),
            base_unit_price: amount("1"),
            modifier_unit_delta: delta("0"),
            effective_unit_price: amount("1"),
            quantity: 0,
            position: 0,
        }),
        Err(SaleError::InvalidQuantity)
    );
    assert_eq!(
        SaleItem::create(NewSaleItem {
            id: id("00000000-0000-0000-0000-000000000642"),
            organization_id: scope().organization_id,
            sale_id: sale().id().clone(),
            product_id: id("00000000-0000-0000-0000-000000000643"),
            product_sku_snapshot: sku("SKU-1"),
            product_name_snapshot: name("Café"),
            base_unit_price: amount("1"),
            modifier_unit_delta: delta("0"),
            effective_unit_price: amount("1"),
            quantity: 1_000_001,
            position: 0,
        }),
        Err(SaleError::InvalidQuantity)
    );
    assert_eq!(
        SaleItem::create(NewSaleItem {
            id: id("00000000-0000-0000-0000-000000000644"),
            organization_id: scope().organization_id,
            sale_id: sale().id().clone(),
            product_id: id("00000000-0000-0000-0000-000000000645"),
            product_sku_snapshot: sku("SKU-1"),
            product_name_snapshot: name("Café"),
            base_unit_price: amount("1"),
            modifier_unit_delta: delta("0"),
            effective_unit_price: amount("999999999999999.9999"),
            quantity: 2,
            position: 0,
        }),
        Err(SaleError::AmountOutOfRange)
    );
}

#[test]
fn removed_items_stop_counting_and_cannot_be_edited() {
    let mut active = item(0, "10", 2);
    let mut removed = item(1, "5", 1);
    removed
        .remove(actor(), moment(1_800_000_000))
        .unwrap_or_else(|error| panic!("retirar: {error}"));
    assert_eq!(removed.status(), SaleItemStatus::Removed);
    assert_eq!(
        removed.remove(actor(), moment(1_800_000_100)),
        Err(SaleError::ItemAlreadyRemoved)
    );
    assert_eq!(
        removed.update_quote(amount("5"), delta("0"), amount("5"), 1),
        Err(SaleError::ItemAlreadyRemoved)
    );

    assert_eq!(
        active
            .update_quote(amount("10"), delta("0"), amount("12"), 4)
            .map(|()| active.line_total().to_string()),
        Ok("48.0000".to_owned())
    );
    assert_eq!(
        total_of_active_items(&[active, removed])
            .unwrap_or_else(|error| panic!("total: {error}"))
            .to_string(),
        "48.0000",
        "solo las líneas activas suman"
    );
}

#[test]
fn payments_enforce_cash_and_card_contracts() {
    let cash = Payment::create(NewPayment {
        id: id("00000000-0000-0000-0000-000000000650"),
        scope: scope(),
        sale_id: sale().id().clone(),
        position: 1,
        method: PaymentMethod::Cash,
        amount: amount("70"),
        tendered_amount: Some(amount("100")),
        created_by_user_id: actor(),
    })
    .unwrap_or_else(|error| panic!("efectivo: {error}"));
    assert_eq!(cash.change_amount().to_string(), "30.0000");
    assert_eq!(
        cash.tendered_amount().map(|value| value.to_string()),
        Some("100.0000".to_owned())
    );

    assert_eq!(
        Payment::create(NewPayment {
            id: id("00000000-0000-0000-0000-000000000651"),
            scope: scope(),
            sale_id: sale().id().clone(),
            position: 1,
            method: PaymentMethod::Cash,
            amount: amount("70"),
            tendered_amount: None,
            created_by_user_id: actor(),
        }),
        Err(SaleError::CashTenderedRequired)
    );
    assert_eq!(
        Payment::create(NewPayment {
            id: id("00000000-0000-0000-0000-000000000652"),
            scope: scope(),
            sale_id: sale().id().clone(),
            position: 1,
            method: PaymentMethod::Cash,
            amount: amount("70"),
            tendered_amount: Some(amount("69.9999")),
            created_by_user_id: actor(),
        }),
        Err(SaleError::CashTenderedBelowAmount)
    );
    assert_eq!(
        Payment::create(NewPayment {
            id: id("00000000-0000-0000-0000-000000000653"),
            scope: scope(),
            sale_id: sale().id().clone(),
            position: 1,
            method: PaymentMethod::Card,
            amount: amount("70"),
            tendered_amount: Some(amount("70")),
            created_by_user_id: actor(),
        }),
        Err(SaleError::CardCannotTender)
    );
    let card = Payment::create(NewPayment {
        id: id("00000000-0000-0000-0000-000000000654"),
        scope: scope(),
        sale_id: sale().id().clone(),
        position: 2,
        method: PaymentMethod::Card,
        amount: amount("70"),
        tendered_amount: None,
        created_by_user_id: actor(),
    })
    .unwrap_or_else(|error| panic!("tarjeta: {error}"));
    assert_eq!(card.change_amount().to_string(), "0.0000");
    assert_eq!(card.tendered_amount(), None);

    assert_eq!(
        Payment::create(NewPayment {
            id: id("00000000-0000-0000-0000-000000000655"),
            scope: scope(),
            sale_id: sale().id().clone(),
            position: 1,
            method: PaymentMethod::Cash,
            amount: amount("0"),
            tendered_amount: Some(amount("0")),
            created_by_user_id: actor(),
        }),
        Err(SaleError::PaymentMustBePositive)
    );
}

#[test]
fn checkout_requires_exact_payment_coverage() {
    let cash = CheckoutInstruction {
        method: PaymentMethod::Cash,
        amount: amount("70"),
        tendered_amount: Some(amount("100")),
    };
    let single_cash = [cash.clone()];
    assert_eq!(validate_checkout(amount("70"), 1, &single_cash), Ok(()));
    assert_eq!(
        validate_checkout(amount("70"), 0, &single_cash),
        Err(SaleError::NoActiveItems)
    );
    assert_eq!(
        validate_checkout(amount("70"), 1, &[]),
        Err(SaleError::PaymentTotalMismatch)
    );
    assert_eq!(
        validate_checkout(amount("0"), 1, &single_cash),
        Err(SaleError::PaymentTotalMismatch),
        "una venta de cero no admite pagos"
    );
    assert_eq!(
        validate_checkout(amount("0"), 1, &[]),
        Ok(()),
        "una venta de cero se cobra sin pagos"
    );
    let short = CheckoutInstruction {
        method: PaymentMethod::Card,
        amount: amount("69.9999"),
        tendered_amount: None,
    };
    assert_eq!(
        validate_checkout(amount("70"), 1, &[short]),
        Err(SaleError::PaymentTotalMismatch)
    );
    let over = CheckoutInstruction {
        method: PaymentMethod::Card,
        amount: amount("70.0001"),
        tendered_amount: None,
    };
    assert_eq!(
        validate_checkout(amount("70"), 1, &[over]),
        Err(SaleError::PaymentTotalMismatch),
        "tampoco se admite cobrar de más"
    );
    let second_cash = CheckoutInstruction {
        method: PaymentMethod::Cash,
        amount: amount("1"),
        tendered_amount: Some(amount("1")),
    };
    assert_eq!(
        validate_checkout(amount("71"), 1, &[cash, second_cash]),
        Err(SaleError::MultipleCashPayments)
    );
}

#[test]
fn checkout_rejects_more_than_ten_payments() {
    let instructions: Vec<CheckoutInstruction> = (0..11)
        .map(|_| CheckoutInstruction {
            method: PaymentMethod::Card,
            amount: amount("1"),
            tendered_amount: None,
        })
        .collect();
    assert_eq!(
        validate_checkout(amount("11"), 1, &instructions),
        Err(SaleError::TooManyPayments)
    );
}

fn group(value: &str, active: bool, min: u32, max: u32) -> ModifierGroupAssignment {
    ModifierGroupAssignment {
        group_id: id(value),
        active,
        min_selections: min,
        max_selections: max,
    }
}

fn option(
    group: &str,
    option_id: &str,
    active: bool,
    price: &str,
    order: u32,
) -> SelectedModifierOption {
    SelectedModifierOption {
        group_id: id(group),
        option_id: id(option_id),
        active,
        price_delta: delta(price),
        display_order: order,
    }
}

#[test]
fn modifier_pricing_sums_deltas_and_orders_the_selection() {
    let assignments = vec![
        group("00000000-0000-0000-0000-000000000701", true, 1, 1),
        group("00000000-0000-0000-0000-000000000702", true, 0, 2),
    ];
    let selected = vec![
        option(
            "00000000-0000-0000-0000-000000000702",
            "00000000-0000-0000-0000-000000000712",
            true,
            "2.5",
            2,
        ),
        option(
            "00000000-0000-0000-0000-000000000701",
            "00000000-0000-0000-0000-000000000711",
            true,
            "-5",
            0,
        ),
        option(
            "00000000-0000-0000-0000-000000000702",
            "00000000-0000-0000-0000-000000000710",
            true,
            "1",
            1,
        ),
    ];

    let pricing = resolve_modifier_pricing(amount("35"), &assignments, &selected)
        .unwrap_or_else(|error| panic!("cotización: {error}"));
    assert_eq!(pricing.delta.to_string(), "-1.5000");
    assert_eq!(pricing.effective_unit_price.to_string(), "33.5000");
    assert_eq!(
        pricing.ordered,
        vec![
            (
                id("00000000-0000-0000-0000-000000000701"),
                id("00000000-0000-0000-0000-000000000711")
            ),
            (
                id("00000000-0000-0000-0000-000000000702"),
                id("00000000-0000-0000-0000-000000000710")
            ),
            (
                id("00000000-0000-0000-0000-000000000702"),
                id("00000000-0000-0000-0000-000000000712")
            ),
        ],
        "grupos por asignación y opciones por display_order"
    );
}

#[test]
fn modifier_pricing_enforces_selection_rules() {
    let required = vec![group("00000000-0000-0000-0000-000000000701", true, 1, 1)];
    assert_eq!(
        resolve_modifier_pricing(amount("10"), &required, &[]),
        Err(SaleError::ModifierSelectionOutOfBounds),
        "el grupo obligatorio exige selección"
    );
    assert_eq!(
        resolve_modifier_pricing(
            amount("10"),
            &required,
            &[
                option(
                    "00000000-0000-0000-0000-000000000701",
                    "00000000-0000-0000-0000-000000000711",
                    true,
                    "1",
                    0
                ),
                option(
                    "00000000-0000-0000-0000-000000000701",
                    "00000000-0000-0000-0000-000000000712",
                    true,
                    "1",
                    1
                ),
            ],
        ),
        Err(SaleError::ModifierSelectionOutOfBounds),
        "el grupo simple no admite dos opciones"
    );
    let duplicate = option(
        "00000000-0000-0000-0000-000000000701",
        "00000000-0000-0000-0000-000000000711",
        true,
        "1",
        0,
    );
    assert_eq!(
        resolve_modifier_pricing(amount("10"), &required, &[duplicate.clone(), duplicate]),
        Err(SaleError::DuplicateModifierOption)
    );
    assert_eq!(
        resolve_modifier_pricing(
            amount("10"),
            &required,
            &[option(
                "00000000-0000-0000-0000-000000000799",
                "00000000-0000-0000-0000-000000000711",
                true,
                "1",
                0
            )],
        ),
        Err(SaleError::ModifierOptionNotAssigned)
    );
    assert_eq!(
        resolve_modifier_pricing(
            amount("10"),
            &required,
            &[option(
                "00000000-0000-0000-0000-000000000701",
                "00000000-0000-0000-0000-000000000711",
                false,
                "1",
                0
            )],
        ),
        Err(SaleError::ModifierOptionInactive)
    );
}

#[test]
fn inactive_groups_are_only_allowed_without_selection() {
    let optional_inactive = vec![group("00000000-0000-0000-0000-000000000703", false, 0, 2)];
    let pricing = resolve_modifier_pricing(amount("10"), &optional_inactive, &[])
        .unwrap_or_else(|error| panic!("grupo opcional inactivo: {error}"));
    assert_eq!(pricing.delta.to_string(), "0.0000");
    assert_eq!(pricing.effective_unit_price.to_string(), "10.0000");

    assert_eq!(
        resolve_modifier_pricing(
            amount("10"),
            &optional_inactive,
            &[option(
                "00000000-0000-0000-0000-000000000703",
                "00000000-0000-0000-0000-000000000713",
                true,
                "1",
                0
            )],
        ),
        Err(SaleError::ModifierGroupInactiveSelection)
    );
    let required_inactive = vec![group("00000000-0000-0000-0000-000000000704", false, 1, 1)];
    assert_eq!(
        resolve_modifier_pricing(amount("10"), &required_inactive, &[]),
        Err(SaleError::RequiredModifierGroupInactive)
    );
}

#[test]
fn modifier_pricing_rejects_a_negative_effective_price() {
    let assignments = vec![group("00000000-0000-0000-0000-000000000705", true, 1, 1)];
    assert_eq!(
        resolve_modifier_pricing(
            amount("2"),
            &assignments,
            &[option(
                "00000000-0000-0000-0000-000000000705",
                "00000000-0000-0000-0000-000000000715",
                true,
                "-3",
                0
            )],
        ),
        Err(SaleError::NegativeEffectivePrice)
    );
}

#[test]
fn restored_entities_reject_incoherent_state() {
    assert_eq!(SaleStatus::parse("COMPLETED"), Ok(SaleStatus::Completed));
    assert_eq!(
        SaleStatus::parse("PARTIAL"),
        Err(SaleError::InvalidPersistedValue)
    );
    assert_eq!(
        SaleItemStatus::parse("REMOVED"),
        Ok(SaleItemStatus::Removed)
    );
    assert_eq!(
        PaymentMethod::parse("TRANSFER"),
        Err(SaleError::InvalidPersistedValue)
    );

    assert_eq!(
        Sale::restore(
            id("00000000-0000-0000-0000-000000000660"),
            scope(),
            actor(),
            None,
            None,
            SaleStatus::Completed,
            amount("10"),
            None,
            None,
            1,
        ),
        Err(SaleError::InvalidPersistedValue),
        "COMPLETED exige actor e instante de cobro"
    );
    assert!(
        Sale::restore(
            id("00000000-0000-0000-0000-000000000661"),
            scope(),
            actor(),
            None,
            None,
            SaleStatus::Open,
            amount("0"),
            None,
            None,
            0,
        )
        .is_ok()
    );
    assert_eq!(code("cafe").as_str(), "cafe");
}
