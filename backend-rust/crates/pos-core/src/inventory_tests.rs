//! Pruebas de las reglas puras de inventario.
#![allow(clippy::panic)]

use crate::{
    catalog::{CatalogCode, Description},
    identity::UtcSeconds,
    inventory::{
        ConsumptionRequirement, InventoryBaseUnit, InventoryError, InventoryItem,
        InventoryItemPatch, InventoryMovement, InventoryMovementType, MovementReason,
        NewInventoryItem, NewMovement, Quantity, StockBalance, ensure_opening_allowed,
        ensure_sufficient, inventory_item_changed_fields, shortages,
    },
    organization_branch::{DisplayName, EntityId, NegativeStockPolicy, Status},
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn org() -> EntityId {
    id("00000000-0000-0000-0000-000000000401")
}
fn branch() -> EntityId {
    id("00000000-0000-0000-0000-000000000402")
}
fn item_id(value: &str) -> EntityId {
    id(value)
}
fn actor() -> EntityId {
    id("00000000-0000-0000-0000-000000000404")
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
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture instante: {error}"))
}
fn item() -> InventoryItem {
    InventoryItem::create(NewInventoryItem {
        id: item_id("00000000-0000-0000-0000-000000000410"),
        organization_id: org(),
        code: code("leche-entera"),
        name: name("Leche entera"),
        description: None,
        base_unit: InventoryBaseUnit::Milliliter,
        fractional_unit_allowed: true,
    })
}
fn movement_new() -> NewMovement {
    NewMovement {
        id: id("00000000-0000-0000-0000-000000000420"),
        organization_id: org(),
        branch_id: branch(),
        inventory_item_id: item().id().clone(),
        actor_user_id: actor(),
        occurred_at: moment(1_800_000_000),
    }
}

#[test]
fn quantity_parses_exact_decimals_and_rejects_non_canonical_syntax() {
    assert_eq!(qty("0").as_minor(), 0);
    assert_eq!(qty("5").as_minor(), 5_000_000);
    assert_eq!(qty("2.5").as_minor(), 2_500_000);
    assert_eq!(qty("-2.5").as_minor(), -2_500_000);
    assert_eq!(qty("00012.25").as_minor(), 12_250_000);
    assert_eq!(
        qty("9999999999999.999999").as_minor(),
        9_999_999_999_999_999_999
    );

    for invalid in [
        "",
        "+1",
        ".5",
        "5.",
        "1.1234567",
        "1.0000000",
        "1e3",
        "1,5",
        " 1",
        "1 ",
        "abc",
        "--1",
        "1.2.3",
        "-",
    ] {
        assert_eq!(
            Quantity::parse(invalid),
            Err(InventoryError::InvalidQuantity),
            "cantidad aceptada indebidamente: {invalid}"
        );
    }
    assert_eq!(
        Quantity::parse("10000000000000"),
        Err(InventoryError::InvalidQuantity),
        "14 dígitos enteros exceden NUMERIC(19,6)"
    );
}

#[test]
fn quantity_round_trips_displays_and_guards_the_sqlite_range() {
    assert_eq!(qty("2.5").to_string(), "2.500000");
    assert_eq!(qty("0").to_string(), "0.000000");
    assert_eq!(qty("-0.000001").to_string(), "-0.000001");
    assert!(qty("0.000001").is_fractional());
    assert!(!qty("2.000000").is_fractional());

    let near = qty("9223372036854.775807");
    assert_eq!(near.as_minor_i64(), Ok(i64::MAX));
    let beyond = qty("9223372036854.775808");
    assert_eq!(
        beyond.as_minor_i64(),
        Err(InventoryError::InvalidQuantity),
        "el rango lógico supera el INTEGER de SQLite"
    );
}

#[test]
fn quantity_addition_is_checked_and_exact() {
    assert_eq!(qty("1.5").checked_add(&qty("2.25")), Ok(qty("3.75")));
    assert_eq!(
        qty("9999999999999.999999").checked_add(&qty("0.000001")),
        Err(InventoryError::QuantityOverflow)
    );
    assert_eq!(qty("2.5").checked_neg(), Ok(qty("-2.5")));
}

#[test]
fn inventory_item_patch_respects_the_unit_lock() {
    let mut item = item();
    assert_eq!(item.revision(), 0);
    assert!(!item.unit_locked());

    assert_eq!(
        item.patch(InventoryItemPatch {
            expected_version: 0,
            name: Some(name("Leche entera")),
            description: crate::catalog::PatchField::Absent,
            base_unit: Some(InventoryBaseUnit::Milliliter),
            fractional_unit_allowed: None,
            status: None,
        }),
        Ok(false),
        "un patch sin cambios no incrementa la revisión"
    );

    let before = item.clone();
    assert_eq!(
        item.patch(InventoryItemPatch {
            expected_version: 0,
            name: None,
            description: crate::catalog::PatchField::Set(
                Description::new("  Lácteo  ")
                    .unwrap_or_else(|error| panic!("descripción: {error}"))
                    .unwrap_or_else(|| panic!("descripción presente")),
            ),
            base_unit: Some(InventoryBaseUnit::Gram),
            fractional_unit_allowed: Some(false),
            status: Some(Status::Inactive),
        }),
        Ok(true)
    );
    assert_eq!(item.base_unit(), InventoryBaseUnit::Gram);
    assert_eq!(
        inventory_item_changed_fields(&before, &item),
        vec!["description", "baseUnit", "fractionalUnitAllowed", "status"]
    );

    assert!(item.mark_unit_locked());
    assert!(!item.mark_unit_locked(), "el bloqueo es monótono");
    let locked_revision = item.revision();
    assert_eq!(
        item.patch(InventoryItemPatch {
            expected_version: locked_revision,
            name: None,
            description: crate::catalog::PatchField::Absent,
            base_unit: Some(InventoryBaseUnit::Unit),
            fractional_unit_allowed: None,
            status: None,
        }),
        Err(InventoryError::UnitLocked)
    );
    assert_eq!(item.base_unit(), InventoryBaseUnit::Gram);
    assert_eq!(
        item.revision(),
        locked_revision,
        "un patch rechazado no deja cambios parciales"
    );
}

#[test]
fn unit_rule_applies_to_the_resulting_quantity() {
    let discrete = InventoryItem::create(NewInventoryItem {
        id: item_id("00000000-0000-0000-0000-000000000411"),
        organization_id: org(),
        code: code("vaso"),
        name: name("Vaso"),
        description: None,
        base_unit: InventoryBaseUnit::Unit,
        fractional_unit_allowed: false,
    });
    assert_eq!(discrete.ensure_quantity_fits_unit(qty("3")), Ok(()));
    assert_eq!(
        discrete.ensure_quantity_fits_unit(qty("2.5")),
        Err(InventoryError::FractionalQuantityNotAllowed)
    );

    let allowed = InventoryItem::create(NewInventoryItem {
        id: item_id("00000000-0000-0000-0000-000000000412"),
        organization_id: org(),
        code: code("gramo"),
        name: name("Gramo"),
        description: None,
        base_unit: InventoryBaseUnit::Unit,
        fractional_unit_allowed: true,
    });
    assert_eq!(allowed.ensure_quantity_fits_unit(qty("2.5")), Ok(()));

    let measured = InventoryItem::create(NewInventoryItem {
        id: item_id("00000000-0000-0000-0000-000000000413"),
        organization_id: org(),
        code: code("agua"),
        name: name("Agua"),
        description: None,
        base_unit: InventoryBaseUnit::Milliliter,
        fractional_unit_allowed: false,
    });
    assert_eq!(
        measured.ensure_quantity_fits_unit(qty("0.5")),
        Ok(()),
        "GRAM y MILLILITER no exigen enteros"
    );
}

#[test]
fn stock_balance_applies_deltas_under_cas() {
    let mut balance = StockBalance::empty(
        id("00000000-0000-0000-0000-000000000430"),
        org(),
        branch(),
        item_id("00000000-0000-0000-0000-000000000410"),
    );
    assert_eq!(balance.quantity_on_hand(), qty("0"));
    assert_eq!(
        balance.ensure_scope(
            &org(),
            &branch(),
            &item_id("00000000-0000-0000-0000-000000000410")
        ),
        Ok(())
    );
    assert_eq!(
        balance.apply_delta(0, qty("10")),
        Ok(qty("10")),
        "la apertura en cero también genera versión nueva"
    );
    assert_eq!(balance.revision(), 1);
    assert_eq!(balance.apply_delta(1, qty("-2.5")), Ok(qty("7.5")));
    assert_eq!(balance.revision(), 2);
    assert_eq!(
        balance.apply_delta(1, qty("1")),
        Err(InventoryError::ConcurrentModification)
    );
    assert_eq!(balance.quantity_on_hand(), qty("7.5"));
    assert_eq!(
        balance.apply_delta(2, qty("9999999999999.999999")),
        Err(InventoryError::QuantityOverflow)
    );
}

#[test]
fn movement_types_enforce_their_signature() {
    assert_eq!(
        InventoryMovement::opening(movement_new(), qty("-1"), qty("0")),
        Err(InventoryError::OpeningMustBeNonNegative)
    );
    let opening = InventoryMovement::opening(movement_new(), qty("0"), qty("0"))
        .unwrap_or_else(|error| panic!("apertura: {error}"));
    assert_eq!(opening.movement_type(), InventoryMovementType::Opening);
    assert_eq!(opening.reason(), None);
    assert_eq!(opening.sale_id(), None);

    let reason = MovementReason::new("  merma  ").unwrap_or_else(|error| panic!("motivo: {error}"));
    assert_eq!(reason.as_str(), "merma");
    let adjustment = InventoryMovement::adjustment(movement_new(), qty("-1"), qty("-1"), reason)
        .unwrap_or_else(|error| panic!("ajuste: {error}"));
    assert_eq!(adjustment.quantity_delta(), qty("-1"));
    assert_eq!(
        adjustment.reason().map(MovementReason::as_str),
        Some("merma")
    );

    assert_eq!(
        InventoryMovement::adjustment(
            movement_new(),
            qty("0"),
            qty("0"),
            MovementReason::new("x").unwrap_or_else(|error| panic!("{error}")),
        ),
        Err(InventoryError::AdjustmentMustNotBeZero)
    );
    assert_eq!(
        InventoryMovement::sale_consumption(
            movement_new(),
            qty("1"),
            qty("0"),
            id("00000000-0000-0000-0000-000000000440")
        ),
        Err(InventoryError::SaleConsumptionMustBeNegative)
    );
    assert_eq!(
        InventoryMovement::purchase_receipt(
            movement_new(),
            qty("0"),
            qty("0"),
            id("00000000-0000-0000-0000-000000000441")
        ),
        Err(InventoryError::PurchaseReceiptMustBePositive)
    );
}

#[test]
fn movement_reason_is_bounded_and_never_blank() {
    assert_eq!(
        MovementReason::new("   "),
        Err(InventoryError::ReasonRequired)
    );
    assert!(MovementReason::new(&"r".repeat(255)).is_ok());
    assert_eq!(
        MovementReason::new(&"r".repeat(256)),
        Err(InventoryError::ReasonTooLong)
    );
    assert_eq!(
        MovementReason::restore(" merma"),
        Err(InventoryError::InvalidPersistedValue)
    );
}

#[test]
fn restored_movements_reject_incoherent_types() {
    let reason = MovementReason::new("ajuste").unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        InventoryMovement::restore(
            id("00000000-0000-0000-0000-000000000421"),
            org(),
            branch(),
            item_id("00000000-0000-0000-0000-000000000410"),
            InventoryMovementType::Opening,
            qty("-1"),
            qty("0"),
            None,
            actor(),
            moment(1_800_000_000),
            None,
            None,
        ),
        Err(InventoryError::InvalidPersistedValue),
        "OPENING no puede ser negativo"
    );
    assert_eq!(
        InventoryMovement::restore(
            id("00000000-0000-0000-0000-000000000422"),
            org(),
            branch(),
            item_id("00000000-0000-0000-0000-000000000410"),
            InventoryMovementType::Adjustment,
            qty("1"),
            qty("1"),
            None,
            actor(),
            moment(1_800_000_000),
            None,
            None,
        ),
        Err(InventoryError::InvalidPersistedValue),
        "ADJUSTMENT exige motivo"
    );
    assert_eq!(
        InventoryMovement::restore(
            id("00000000-0000-0000-0000-000000000423"),
            org(),
            branch(),
            item_id("00000000-0000-0000-0000-000000000410"),
            InventoryMovementType::SaleConsumption,
            qty("-1"),
            qty("0"),
            Some(reason.clone()),
            actor(),
            moment(1_800_000_000),
            Some(id("00000000-0000-0000-0000-000000000440")),
            None,
        ),
        Err(InventoryError::InvalidPersistedValue),
        "SALE_CONSUMPTION no admite motivo"
    );
    assert!(
        InventoryMovement::restore(
            id("00000000-0000-0000-0000-000000000424"),
            org(),
            branch(),
            item_id("00000000-0000-0000-0000-000000000410"),
            InventoryMovementType::PurchaseReceipt,
            qty("5"),
            qty("5"),
            None,
            actor(),
            moment(1_800_000_000),
            None,
            Some(id("00000000-0000-0000-0000-000000000441")),
        )
        .is_ok()
    );
}

#[test]
fn opening_rules_match_the_java_order() {
    assert_eq!(ensure_opening_allowed(false, false), Ok(()));
    assert_eq!(
        ensure_opening_allowed(true, true),
        Err(InventoryError::OpeningAlreadyExists)
    );
    assert_eq!(
        ensure_opening_allowed(false, true),
        Err(InventoryError::HistoryAlreadyExists)
    );
}

#[test]
fn sufficiency_is_all_or_nothing_under_strict_policy() {
    let milk = item_id("00000000-0000-0000-0000-000000000410");
    let cups = item_id("00000000-0000-0000-0000-000000000411");
    let balances = vec![(milk.clone(), qty("10")), (cups.clone(), qty("1"))];
    let requirements = vec![
        ConsumptionRequirement {
            inventory_item_id: milk.clone(),
            inventory_item_code: code("leche-entera"),
            required_quantity: qty("2"),
        },
        ConsumptionRequirement {
            inventory_item_id: cups.clone(),
            inventory_item_code: code("vaso"),
            required_quantity: qty("3"),
        },
    ];

    assert_eq!(
        ensure_sufficient(NegativeStockPolicy::Strict, &balances, &requirements),
        Err(InventoryError::InsufficientInventory)
    );
    let found = shortages(&balances, &requirements);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].inventory_item_code.as_str(), "vaso");
    assert_eq!(found[0].required, qty("3"));
    assert_eq!(found[0].available, qty("1"));

    assert_eq!(
        ensure_sufficient(NegativeStockPolicy::AllowNegative, &balances, &requirements),
        Ok(()),
        "ALLOW_NEGATIVE permite continuar con faltantes"
    );

    let sufficient = vec![ConsumptionRequirement {
        inventory_item_id: milk,
        inventory_item_code: code("leche-entera"),
        required_quantity: qty("10"),
    }];
    assert_eq!(
        ensure_sufficient(NegativeStockPolicy::Strict, &balances, &sufficient),
        Ok(()),
        "el saldo exacto es suficiente"
    );
}
