//! Pruebas de recetas, efectos de modificadores y resolución de consumo.
#![allow(clippy::panic)]

use crate::{
    inventory::{InventoryBaseUnit, Quantity},
    organization_branch::EntityId,
    recipe::{
        EffectChange, RecipeChange, RecipeError, RecipeStatus, ResolvedComponent, ResolvedEffect,
        SaleInventorySnapshot, accumulate_line_totals, plan_effect_replacement,
        plan_recipe_replacement, resolve_consumption, snapshot_line_consumption,
    },
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn milk() -> EntityId {
    id("00000000-0000-0000-0000-000000000901")
}
fn cup() -> EntityId {
    id("00000000-0000-0000-0000-000000000902")
}
fn qty(value: &str) -> Quantity {
    Quantity::parse(value).unwrap_or_else(|error| panic!("fixture cantidad: {error}"))
}
fn component(item: EntityId, unit: InventoryBaseUnit, quantity: &str) -> ResolvedComponent {
    ResolvedComponent {
        inventory_item_id: item,
        base_unit: unit,
        fractional_unit_allowed: true,
        active: true,
        quantity_per_product: qty(quantity),
    }
}
fn effect(item: EntityId, quantity: &str) -> ResolvedEffect {
    ResolvedEffect {
        inventory_item_id: item,
        base_unit: InventoryBaseUnit::Milliliter,
        fractional_unit_allowed: true,
        active: true,
        quantity_delta: qty(quantity),
    }
}

#[test]
fn active_recipe_requires_components_and_rejects_duplicates() {
    assert_eq!(
        plan_recipe_replacement(None, &[], RecipeStatus::Active, &[]),
        Err(RecipeError::ActiveRecipeRequiresComponents)
    );
    let duplicated = vec![
        component(milk(), InventoryBaseUnit::Milliliter, "150"),
        component(milk(), InventoryBaseUnit::Milliliter, "150"),
    ];
    assert_eq!(
        plan_recipe_replacement(None, &[], RecipeStatus::Active, &duplicated),
        Err(RecipeError::DuplicateComponent)
    );
    assert_eq!(
        plan_recipe_replacement(
            None,
            &[],
            RecipeStatus::Active,
            &[component(milk(), InventoryBaseUnit::Milliliter, "0")],
        ),
        Err(RecipeError::QuantityMustBePositive)
    );
    assert_eq!(
        plan_recipe_replacement(
            None,
            &[],
            RecipeStatus::Active,
            &[component(milk(), InventoryBaseUnit::Milliliter, "-5")],
        ),
        Err(RecipeError::QuantityMustBePositive)
    );
}

#[test]
fn inactive_recipe_may_be_empty() {
    assert_eq!(
        plan_recipe_replacement(Some(RecipeStatus::Active), &[], RecipeStatus::Inactive, &[]),
        Ok(RecipeChange::Replace {
            status: RecipeStatus::Inactive,
            components: vec![],
        }),
        "desactivar una receta la vacía sin borrar la fila"
    );
}

#[test]
fn unit_rule_applies_to_recipe_components() {
    let discrete = ResolvedComponent {
        inventory_item_id: cup(),
        base_unit: InventoryBaseUnit::Unit,
        fractional_unit_allowed: false,
        active: true,
        quantity_per_product: qty("1.5"),
    };
    assert_eq!(
        plan_recipe_replacement(None, &[], RecipeStatus::Active, &[discrete]),
        Err(RecipeError::FractionalQuantityNotAllowed)
    );
}

#[test]
fn inactive_items_are_only_allowed_when_retained_unchanged() {
    let prior = vec![ResolvedComponent {
        inventory_item_id: milk(),
        base_unit: InventoryBaseUnit::Milliliter,
        fractional_unit_allowed: true,
        active: false,
        quantity_per_product: qty("150"),
    }];
    assert_eq!(
        plan_recipe_replacement(
            Some(RecipeStatus::Active),
            &prior,
            RecipeStatus::Inactive,
            &prior,
        )
        .map(|change| matches!(change, RecipeChange::Unchanged)),
        Ok(false),
        "el estado cambia aunque el componente se conserve"
    );
    assert_eq!(
        plan_recipe_replacement(
            Some(RecipeStatus::Active),
            &prior,
            RecipeStatus::Active,
            &prior
        ),
        Err(RecipeError::InactiveItemNotAllowed),
        "una receta activa no puede conservar un artículo inactivo"
    );
    let changed = vec![ResolvedComponent {
        quantity_per_product: qty("200"),
        ..prior[0].clone()
    }];
    assert_eq!(
        plan_recipe_replacement(
            Some(RecipeStatus::Active),
            &prior,
            RecipeStatus::Inactive,
            &changed,
        ),
        Err(RecipeError::InactiveItemNotAllowed),
        "cambiar la cantidad de un artículo inactivo no es una retención"
    );
    assert_eq!(
        plan_recipe_replacement(
            None,
            &[],
            RecipeStatus::Inactive,
            &[component(cup(), InventoryBaseUnit::Unit, "1")],
        )
        .map(|change| matches!(change, RecipeChange::Replace { .. })),
        Ok(true)
    );
}

#[test]
fn identical_recipe_is_a_no_op() {
    let components = vec![
        component(milk(), InventoryBaseUnit::Milliliter, "150"),
        component(cup(), InventoryBaseUnit::Unit, "1"),
    ];
    assert_eq!(
        plan_recipe_replacement(
            Some(RecipeStatus::Active),
            &components,
            RecipeStatus::Active,
            &components,
        ),
        Ok(RecipeChange::Unchanged)
    );
    let reordered = vec![components[1].clone(), components[0].clone()];
    assert!(
        matches!(
            plan_recipe_replacement(
                Some(RecipeStatus::Active),
                &components,
                RecipeStatus::Active,
                &reordered,
            ),
            Ok(RecipeChange::Replace { .. })
        ),
        "un orden distinto es un cambio real"
    );
}

#[test]
fn effect_plans_reject_zero_deltas_duplicates_and_inactive_items() {
    assert_eq!(
        plan_effect_replacement(&[], &[effect(milk(), "0")]),
        Err(RecipeError::EffectDeltaMustNotBeZero)
    );
    assert_eq!(
        plan_effect_replacement(&[], &[effect(milk(), "10"), effect(milk(), "5")]),
        Err(RecipeError::DuplicateComponent)
    );
    let inactive = vec![ResolvedEffect {
        active: false,
        ..effect(milk(), "10")
    }];
    assert_eq!(
        plan_effect_replacement(&[], &inactive),
        Err(RecipeError::InactiveItemNotAllowed)
    );
    let current = vec![effect(milk(), "-20")];
    assert_eq!(
        plan_effect_replacement(&current, &current),
        Ok(EffectChange::Unchanged)
    );
    assert_eq!(
        plan_effect_replacement(&[], &current),
        Ok(EffectChange::Replace(current.clone()))
    );
}

#[test]
fn consumption_combines_recipe_and_effects_exactly() {
    let components = vec![
        component(milk(), InventoryBaseUnit::Milliliter, "150"),
        component(cup(), InventoryBaseUnit::Unit, "1"),
    ];
    let effects = vec![effect(milk(), "50"), effect(cup(), "-1")];
    let resolved = resolve_consumption(true, &components, &effects)
        .unwrap_or_else(|error| panic!("resolución: {error}"));
    assert!(resolved.tracked);
    assert_eq!(
        resolved.components,
        vec![
            crate::recipe::ResolvedConsumptionComponent {
                inventory_item_id: milk(),
                base_unit: InventoryBaseUnit::Milliliter,
                quantity: qty("200"),
            },
            crate::recipe::ResolvedConsumptionComponent {
                inventory_item_id: cup(),
                base_unit: InventoryBaseUnit::Unit,
                quantity: qty("0"),
            },
        ]
        .into_iter()
        .filter(|component| !component.quantity.is_zero())
        .collect::<Vec<_>>()
    );
}

#[test]
fn untracked_products_resolve_to_no_consumption() {
    let resolved =
        resolve_consumption(false, &[], &[]).unwrap_or_else(|error| panic!("sin receta: {error}"));
    assert!(!resolved.tracked);
    assert!(resolved.components.is_empty());
}

#[test]
fn negative_or_inactive_configuration_is_rejected() {
    let components = vec![component(milk(), InventoryBaseUnit::Milliliter, "10")];
    let effects = vec![effect(cup(), "-1")];
    assert_eq!(
        resolve_consumption(true, &components, &[effect(milk(), "-20")]),
        Err(RecipeError::NegativeResolvedQuantity)
    );
    assert_eq!(
        resolve_consumption(true, &components, &effects),
        Err(RecipeError::NegativeResolvedQuantity),
        "un artículo que solo aparece con delta negativo no puede consumirse"
    );
    let inactive = vec![ResolvedComponent {
        active: false,
        ..component(milk(), InventoryBaseUnit::Milliliter, "10")
    }];
    assert_eq!(
        resolve_consumption(true, &inactive, &[]),
        Err(RecipeError::InactiveItemNotAllowed)
    );
}

#[test]
fn fractional_totals_are_rejected_for_discrete_items() {
    let components = vec![ResolvedComponent {
        inventory_item_id: cup(),
        base_unit: InventoryBaseUnit::Unit,
        fractional_unit_allowed: false,
        active: true,
        quantity_per_product: qty("1"),
    }];
    let resolved = resolve_consumption(true, &components, &[])
        .unwrap_or_else(|error| panic!("entero: {error}"));
    assert_eq!(resolved.components.len(), 1);

    let effects = vec![ResolvedEffect {
        base_unit: InventoryBaseUnit::Unit,
        fractional_unit_allowed: false,
        ..effect(cup(), "0.5")
    }];
    assert_eq!(
        resolve_consumption(true, &components, &effects),
        Err(RecipeError::FractionalQuantityNotAllowed),
        "la regla se aplica al total resuelto"
    );
}

#[test]
fn line_snapshots_multiply_and_accumulate_exactly() {
    let components = vec![component(milk(), InventoryBaseUnit::Milliliter, "150")];
    let resolved = resolve_consumption(true, &components, &[])
        .unwrap_or_else(|error| panic!("resolución: {error}"));
    let snapshots =
        snapshot_line_consumption(&resolved, 3).unwrap_or_else(|error| panic!("snapshot: {error}"));
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].quantity_per_unit, qty("150"));
    assert_eq!(snapshots[0].quantity_total, qty("450"));
    assert_eq!(snapshots[0].base_unit, InventoryBaseUnit::Milliliter);

    let mut all = snapshots.clone();
    all.extend(
        snapshot_line_consumption(&resolved, 2).unwrap_or_else(|error| panic!("snapshot: {error}")),
    );
    let totals =
        accumulate_line_totals(&all).unwrap_or_else(|error| panic!("acumulación: {error}"));
    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].quantity, qty("750"));

    assert_eq!(
        snapshot_line_consumption(&resolved, 0),
        Err(RecipeError::InvalidSaleQuantity)
    );
}

#[test]
fn snapshot_multiplication_detects_overflow() {
    let components = vec![component(
        milk(),
        InventoryBaseUnit::Milliliter,
        "9999999999999.999999",
    )];
    let resolved = resolve_consumption(true, &components, &[])
        .unwrap_or_else(|error| panic!("resolución: {error}"));
    assert_eq!(
        snapshot_line_consumption(&resolved, 2),
        Err(RecipeError::QuantityOutOfRange)
    );
}

#[test]
fn status_parsing_and_restored_snapshots_are_strict() {
    assert_eq!(RecipeStatus::parse("ACTIVE"), Ok(RecipeStatus::Active));
    assert_eq!(
        RecipeStatus::parse("DRAFT"),
        Err(RecipeError::InvalidPersistedValue)
    );
    let snapshot = SaleInventorySnapshot {
        inventory_item_id: milk(),
        base_unit: InventoryBaseUnit::Milliliter,
        quantity_per_unit: qty("1"),
        quantity_total: qty("1"),
    };
    assert_eq!(
        accumulate_line_totals(&[snapshot]).map(|values| values[0].quantity),
        Ok(qty("1"))
    );
}
