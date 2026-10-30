//! Pruebas de las reglas puras del catálogo organizacional.
//!
//! Los casos replican la aceptación y el rechazo del agregado Java
//! `catalog/domain/*`; no tocan SQLite, SQLx ni transporte.
#![allow(clippy::panic)]

use crate::{
    catalog::{
        Barcode, CatalogCode, CatalogError, Category, CategoryPatch, Decimal4, Description,
        ModifierGroup, ModifierGroupPatch, ModifierOption, ModifierOptionPatch,
        ModifierReplacement, NewCategory, NewModifierGroup, NewModifierOption, NewProduct,
        PatchField, PriceDelta, Product, ProductPatch, Sku, UnitPrice, category_changed_fields,
        display_order, modifier_group_changed_fields, modifier_option_changed_fields,
        plan_modifier_replacement, product_changed_fields,
    },
    organization_branch::{DisplayName, EntityId, Status},
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn org() -> EntityId {
    id("00000000-0000-0000-0000-000000000201")
}
fn other_org() -> EntityId {
    id("00000000-0000-0000-0000-000000000202")
}
fn name(value: &str) -> DisplayName {
    DisplayName::new(value).unwrap_or_else(|error| panic!("fixture nombre: {error}"))
}
fn code(value: &str) -> CatalogCode {
    CatalogCode::new(value).unwrap_or_else(|error| panic!("fixture código: {error}"))
}
fn sku(value: &str) -> Sku {
    Sku::new(value).unwrap_or_else(|error| panic!("fixture SKU: {error}"))
}
fn price(value: &str) -> UnitPrice {
    UnitPrice::parse(value).unwrap_or_else(|error| panic!("fixture precio: {error}"))
}
fn category(identifier: &str, owner: EntityId) -> Category {
    Category::create(NewCategory {
        id: id(identifier),
        organization_id: owner,
        code: code("cafeteria"),
        name: name("Cafetería"),
    })
}
fn product() -> Product {
    Product::create(NewProduct {
        id: id("00000000-0000-0000-0000-000000000210"),
        organization_id: org(),
        sku: sku("sku-1"),
        name: name("Café americano"),
        unit_price: price("35.5"),
        barcode: None,
        description: None,
        category: None,
    })
    .unwrap_or_else(|error| panic!("fixture producto: {error}"))
}

#[test]
fn catalog_code_normalizes_like_java_and_rejects_invalid_slugs() {
    assert_eq!(code("  Coffee-Drinks ").as_str(), "coffee-drinks");
    assert_eq!(code("a1-b2").as_str(), "a1-b2");
    for invalid in [
        "", "   ", "-a", "a-", "a--b", "a b", "a_b", "a.b", "café", "A/B",
    ] {
        assert_eq!(
            CatalogCode::new(invalid),
            Err(CatalogError::InvalidCode),
            "código aceptado indebidamente: {invalid}"
        );
    }
    let long = "a".repeat(65);
    assert_eq!(CatalogCode::new(&long), Err(CatalogError::InvalidCode));
    assert_eq!(
        CatalogCode::new(&"a".repeat(64))
            .unwrap_or_else(|error| panic!("código de 64: {error}"))
            .as_str()
            .len(),
        64
    );
}

#[test]
fn catalog_code_and_sku_restore_require_the_canonical_form() {
    assert!(CatalogCode::restore("coffee-drinks").is_ok());
    assert_eq!(
        CatalogCode::restore("Coffee-Drinks"),
        Err(CatalogError::InvalidPersistedValue)
    );
    assert_eq!(
        CatalogCode::restore(" coffee"),
        Err(CatalogError::InvalidPersistedValue)
    );
    assert_eq!(Sku::new(" sku-1 ").as_ref().map(Sku::as_str), Ok("SKU-1"));
    assert!(Sku::restore("SKU-1").is_ok());
    assert_eq!(
        Sku::restore("sku-1"),
        Err(CatalogError::InvalidPersistedValue)
    );
}

#[test]
fn sku_uppercases_and_rejects_out_of_contract_characters() {
    assert_eq!(
        sku("abc.123_x-y").as_str(),
        "ABC.123_X-Y",
        "la normalización conserva puntos, guiones y guiones bajos"
    );
    for invalid in ["", "   ", "SKU 1", "SKU/1", "SKU+1", "sku#1"] {
        assert_eq!(
            Sku::new(invalid),
            Err(CatalogError::InvalidSku),
            "{invalid}"
        );
    }
    assert_eq!(Sku::new(&"A".repeat(65)), Err(CatalogError::InvalidSku));
    // Paridad Java: `ß` se expande a `SS` antes de validar.
    assert_eq!(
        Sku::new("ß").as_ref().map(Sku::as_str),
        Ok("SS"),
        "mayúsculas Unicode antes de la gramática ASCII"
    );
}

#[test]
fn optional_text_fields_normalize_blank_to_none_and_bound_length() {
    assert_eq!(
        Barcode::new("   ").unwrap_or_else(|error| panic!("blanco: {error}")),
        None
    );
    assert_eq!(
        Barcode::new(" 0012345678905 ")
            .unwrap_or_else(|error| panic!("barcode: {error}"))
            .as_ref()
            .map(Barcode::as_str),
        Some("0012345678905"),
        "los ceros iniciales se conservan"
    );
    assert_eq!(
        Barcode::new(&"9".repeat(65)),
        Err(CatalogError::InvalidBarcode)
    );
    assert_eq!(
        Description::new("  ").unwrap_or_else(|error| panic!("blanco: {error}")),
        None
    );
    assert_eq!(
        Description::new(" Caliente ")
            .unwrap_or_else(|error| panic!("descripción: {error}"))
            .as_ref()
            .map(Description::as_str),
        Some("Caliente")
    );
    assert_eq!(
        Description::new(&"d".repeat(1001)),
        Err(CatalogError::InvalidDescription)
    );
    assert_eq!(
        Description::new(&"d".repeat(1000))
            .unwrap_or_else(|error| panic!("descripción límite: {error}"))
            .as_ref()
            .map(Description::as_str)
            .map(str::len),
        Some(1000)
    );
}

#[test]
fn decimal4_parses_exact_values_and_rejects_non_canonical_syntax() {
    assert_eq!(price("0").value().as_minor(), 0);
    assert_eq!(price("35.5").value().as_minor(), 355_000);
    assert_eq!(price("12.3456").value().as_minor(), 123_456);
    assert_eq!(price("+7").value().as_minor(), 70_000);
    assert_eq!(price("00012.5").value().as_minor(), 125_000);
    assert_eq!(
        price("123456789012345.9999").value().as_minor(),
        1_234_567_890_123_459_999
    );

    for invalid in [
        "", ".", ".5", "5.", "1.23456", "1.50000", "1E3", "1,5", " 1", "1 ", "abc", "--1", "1.2.3",
    ] {
        assert_eq!(
            UnitPrice::parse(invalid),
            Err(CatalogError::InvalidPrice),
            "precio aceptado indebidamente: {invalid}"
        );
    }
    assert_eq!(UnitPrice::parse("-1"), Err(CatalogError::InvalidPrice));
    assert_eq!(
        UnitPrice::parse("-0.0000").map(|value| value.value().as_minor()),
        Ok(0),
        "el cero negativo no tiene signo distinto para Java"
    );
    assert_eq!(
        UnitPrice::parse("1234567890123456"),
        Err(CatalogError::InvalidPrice),
        "más de 15 dígitos enteros significativos"
    );
}

#[test]
fn decimal4_round_trips_through_display_and_guards_the_sqlite_range() {
    let value = price("1234.5");
    assert_eq!(value.to_string(), "1234.5000");
    assert_eq!(
        Decimal4::parse(&value.to_string()).unwrap_or_else(|error| panic!("ida y vuelta: {error}")),
        value.value()
    );
    assert_eq!(price("0").to_string(), "0.0000");

    let signed = PriceDelta::parse("-2.5").unwrap_or_else(|error| panic!("delta: {error}"));
    assert_eq!(signed.value().as_minor(), -25_000);
    assert_eq!(signed.to_string(), "-2.5000");
    assert!(signed.value().is_negative());

    let near_limit = price("922337203685477.5807");
    assert_eq!(near_limit.value().as_minor_i64(), Ok(i64::MAX));
    let beyond = price("922337203685477.5808");
    assert_eq!(
        beyond.value().as_minor_i64(),
        Err(CatalogError::InvalidPrice),
        "el rango lógico supera el INTEGER de SQLite"
    );
    assert_eq!(
        Decimal4::from_minor(10_000 * 1_000_000_000_000_000),
        Err(CatalogError::InvalidPrice)
    );
}

#[test]
fn display_order_converts_transport_values_without_wrapping() {
    assert_eq!(display_order(0), Ok(0));
    assert_eq!(display_order(7), Ok(7));
    assert_eq!(display_order(-1), Err(CatalogError::InvalidDisplayOrder));
}

#[test]
fn category_patch_is_atomic_and_detects_no_op() {
    let mut category = category("00000000-0000-0000-0000-000000000220", org());
    assert_eq!(category.revision(), 0);
    assert_eq!(category.status(), Status::Active);

    assert_eq!(
        category.patch(CategoryPatch {
            expected_version: 0,
            name: Some(name("Cafetería")),
            status: Some(Status::Active),
        }),
        Ok(false),
        "un patch sin cambios no incrementa la revisión"
    );
    assert_eq!(category.revision(), 0);

    assert_eq!(
        category.patch(CategoryPatch {
            expected_version: 0,
            name: Some(name("  Bebidas  ")),
            status: Some(Status::Inactive),
        }),
        Ok(true)
    );
    assert_eq!(category.name().as_str(), "Bebidas");
    assert_eq!(category.revision(), 1);
    assert!(!category.is_active());

    assert_eq!(
        category.patch(CategoryPatch {
            expected_version: 0,
            name: None,
            status: None,
        }),
        Err(CatalogError::ConcurrentModification)
    );
}

#[test]
fn product_requires_an_active_category_from_the_same_organization() {
    let inactive = {
        let mut value = category("00000000-0000-0000-0000-000000000221", org());
        value
            .patch(CategoryPatch {
                expected_version: 0,
                name: None,
                status: Some(Status::Inactive),
            })
            .unwrap_or_else(|error| panic!("desactivar: {error}"));
        value
    };
    assert_eq!(
        Product::create(NewProduct {
            id: id("00000000-0000-0000-0000-000000000211"),
            organization_id: org(),
            sku: sku("SKU-2"),
            name: name("Producto"),
            unit_price: price("1"),
            barcode: None,
            description: None,
            category: Some(inactive),
        }),
        Err(CatalogError::CategoryInactive)
    );

    let foreign = category("00000000-0000-0000-0000-000000000222", other_org());
    assert_eq!(
        Product::create(NewProduct {
            id: id("00000000-0000-0000-0000-000000000212"),
            organization_id: org(),
            sku: sku("SKU-3"),
            name: name("Producto"),
            unit_price: price("1"),
            barcode: None,
            description: None,
            category: Some(foreign),
        }),
        Err(CatalogError::OwnershipMismatch)
    );

    let active = category("00000000-0000-0000-0000-000000000223", org());
    let with_category = Product::create(NewProduct {
        id: id("00000000-0000-0000-0000-000000000213"),
        organization_id: org(),
        sku: sku("SKU-4"),
        name: name("Producto"),
        unit_price: price("1"),
        barcode: None,
        description: None,
        category: Some(active),
    })
    .unwrap_or_else(|error| panic!("producto con categoría: {error}"));
    assert_eq!(
        with_category.category_id().map(EntityId::as_str),
        Some("00000000-0000-0000-0000-000000000223")
    );
    assert_eq!(with_category.revision(), 0);
}

#[test]
fn product_patch_distinguishes_absent_and_clear_optional_fields() {
    let mut product = product();
    let description = Description::new("Caliente")
        .unwrap_or_else(|error| panic!("descripción: {error}"))
        .unwrap_or_else(|| panic!("descripción presente"));
    let barcode = Barcode::new("7501234567890")
        .unwrap_or_else(|error| panic!("barcode: {error}"))
        .unwrap_or_else(|| panic!("barcode presente"));

    assert_eq!(
        product.patch(ProductPatch {
            expected_version: 0,
            name: None,
            description: PatchField::Set(description.clone()),
            barcode: PatchField::Set(barcode.clone()),
            unit_price: None,
            category: PatchField::Absent,
            status: None,
        }),
        Ok(true)
    );
    assert_eq!(product.revision(), 1);
    assert_eq!(product.description(), Some(&description));
    assert_eq!(product.barcode(), Some(&barcode));

    // Ausente no borra; Clear sí.
    assert_eq!(
        product.patch(ProductPatch {
            expected_version: 1,
            name: None,
            description: PatchField::Absent,
            barcode: PatchField::Clear,
            unit_price: None,
            category: PatchField::Absent,
            status: None,
        }),
        Ok(true)
    );
    assert_eq!(product.description(), Some(&description));
    assert_eq!(product.barcode(), None);
    assert_eq!(product.revision(), 2);

    assert_eq!(
        product.patch(ProductPatch {
            expected_version: 2,
            name: None,
            description: PatchField::Clear,
            barcode: PatchField::Clear,
            unit_price: None,
            category: PatchField::Absent,
            status: None,
        }),
        Ok(true),
        "limpiar un campo ya vacío no cambia, pero limpiar la descripción sí"
    );
    assert_eq!(product.description(), None);
    assert_eq!(product.revision(), 3);
    assert_eq!(product.sku().as_str(), "SKU-1", "el SKU es inmutable");
}

#[test]
fn product_patch_validates_category_assignment_and_reports_changed_fields() {
    let mut product = product();
    let before = product.clone();
    let active = category("00000000-0000-0000-0000-000000000230", org());
    assert_eq!(
        product.patch(ProductPatch {
            expected_version: 0,
            name: None,
            description: PatchField::Absent,
            barcode: PatchField::Absent,
            unit_price: Some(price("40")),
            category: PatchField::Set(active.clone()),
            status: Some(Status::Inactive),
        }),
        Ok(true)
    );
    assert_eq!(
        product_changed_fields(&before, &product),
        vec!["unitPrice", "categoryId", "status"]
    );

    let foreign = category("00000000-0000-0000-0000-000000000231", other_org());
    assert_eq!(
        product.patch(ProductPatch {
            expected_version: 1,
            name: None,
            description: PatchField::Absent,
            barcode: PatchField::Absent,
            unit_price: None,
            category: PatchField::Set(foreign),
            status: None,
        }),
        Err(CatalogError::OwnershipMismatch),
        "un patch rechazado no deja cambios parciales"
    );
    assert_eq!(product.revision(), 1);
    assert_eq!(
        product.category_id().map(EntityId::as_str),
        Some("00000000-0000-0000-0000-000000000230")
    );

    let before_clear = product.clone();
    assert_eq!(
        product.patch(ProductPatch {
            expected_version: 1,
            name: None,
            description: PatchField::Absent,
            barcode: PatchField::Absent,
            unit_price: None,
            category: PatchField::Clear,
            status: None,
        }),
        Ok(true)
    );
    assert_eq!(product.category_id(), None);
    assert_eq!(
        product_changed_fields(&before_clear, &product),
        vec!["categoryId"]
    );
    assert_eq!(
        category_changed_fields(&active, &active),
        Vec::<&str>::new()
    );
}

#[test]
fn product_configuration_change_bumps_the_product_revision() {
    let mut product = product();
    assert_eq!(product.mark_configuration_changed(0), Ok(()));
    assert_eq!(product.revision(), 1);
    assert_eq!(
        product.mark_configuration_changed(0),
        Err(CatalogError::ConcurrentModification)
    );
    assert_eq!(product.revision(), 1);
}

#[test]
fn modifier_group_derives_selection_semantics_and_validates_bounds() {
    let required_single = ModifierGroup::create(NewModifierGroup {
        id: id("00000000-0000-0000-0000-000000000240"),
        organization_id: org(),
        code: code("milk"),
        name: name("Leche"),
        min_selections: 1,
        max_selections: 1,
    })
    .unwrap_or_else(|error| panic!("grupo: {error}"));
    assert!(required_single.is_required());
    assert!(required_single.is_single_choice());
    assert!(!required_single.is_multiple_choice());

    let optional_multi = ModifierGroup::create(NewModifierGroup {
        id: id("00000000-0000-0000-0000-000000000241"),
        organization_id: org(),
        code: code("extras"),
        name: name("Extras"),
        min_selections: 0,
        max_selections: 3,
    })
    .unwrap_or_else(|error| panic!("grupo: {error}"));
    assert!(!optional_multi.is_required());
    assert!(!optional_multi.is_single_choice());
    assert!(optional_multi.is_multiple_choice());

    for (min, max) in [(1, 0), (3, 2), (0, 0)] {
        assert_eq!(
            ModifierGroup::create(NewModifierGroup {
                id: id("00000000-0000-0000-0000-000000000242"),
                organization_id: org(),
                code: code("bad"),
                name: name("Malo"),
                min_selections: min,
                max_selections: max,
            }),
            Err(CatalogError::InvalidSelectionRange),
            "límites ({min}, {max}) aceptados indebidamente"
        );
    }
}

#[test]
fn modifier_group_patch_merges_partial_selection_updates() {
    let mut group = ModifierGroup::create(NewModifierGroup {
        id: id("00000000-0000-0000-0000-000000000250"),
        organization_id: org(),
        code: code("size"),
        name: name("Tamaño"),
        min_selections: 1,
        max_selections: 1,
    })
    .unwrap_or_else(|error| panic!("grupo: {error}"));

    assert_eq!(
        group.patch(ModifierGroupPatch {
            expected_version: 0,
            name: None,
            min_selections: None,
            max_selections: Some(2),
            status: None,
        }),
        Ok(true)
    );
    assert_eq!((group.min_selections(), group.max_selections()), (1, 2));
    assert!(group.is_multiple_choice());
    assert_eq!(group.revision(), 1);

    assert_eq!(
        group.patch(ModifierGroupPatch {
            expected_version: 1,
            name: None,
            min_selections: Some(3),
            max_selections: None,
            status: None,
        }),
        Err(CatalogError::InvalidSelectionRange),
        "el mínimo no puede superar el máximo vigente"
    );
    assert_eq!((group.min_selections(), group.max_selections()), (1, 2));

    let before = group.clone();
    assert_eq!(
        group.patch(ModifierGroupPatch {
            expected_version: 1,
            name: None,
            min_selections: None,
            max_selections: None,
            status: Some(Status::Inactive),
        }),
        Ok(true)
    );
    assert_eq!(
        modifier_group_changed_fields(&before, &group),
        vec!["status"]
    );
}

#[test]
fn modifier_option_derives_ownership_from_its_group() {
    let group = ModifierGroup::create(NewModifierGroup {
        id: id("00000000-0000-0000-0000-000000000260"),
        organization_id: org(),
        code: code("milk"),
        name: name("Leche"),
        min_selections: 0,
        max_selections: 1,
    })
    .unwrap_or_else(|error| panic!("grupo: {error}"));

    let delta = PriceDelta::parse("-1.25").unwrap_or_else(|error| panic!("delta: {error}"));
    let mut option = ModifierOption::create(NewModifierOption {
        id: id("00000000-0000-0000-0000-000000000261"),
        group: group.clone(),
        code: code("skim"),
        name: name("Descremada"),
        price_delta: delta,
        display_order: 2,
    });
    assert_eq!(option.organization_id(), group.organization_id());
    assert_eq!(option.group_id(), group.id());
    assert_eq!(option.price_delta(), delta);
    assert_eq!(option.revision(), 0);

    let before = option.clone();
    assert_eq!(
        option.patch(ModifierOptionPatch {
            expected_version: 0,
            name: None,
            price_delta: Some(PriceDelta::parse("0").unwrap_or_else(|error| panic!("{error}"))),
            display_order: Some(0),
            status: None,
        }),
        Ok(true)
    );
    assert_eq!(
        modifier_option_changed_fields(&before, &option),
        vec!["priceDelta", "displayOrder"]
    );
    assert_eq!(option.revision(), 1);
}

#[test]
fn modifier_replacement_plan_detects_no_op_reordering_and_duplicates() {
    let first = id("00000000-0000-0000-0000-000000000270");
    let second = id("00000000-0000-0000-0000-000000000271");

    assert_eq!(
        plan_modifier_replacement(
            &[first.clone(), second.clone()],
            &[first.clone(), second.clone()]
        ),
        Ok(ModifierReplacement::Unchanged),
        "una lista idéntica no muta ni audita"
    );
    assert_eq!(
        plan_modifier_replacement(
            &[first.clone(), second.clone()],
            &[second.clone(), first.clone()]
        ),
        Ok(ModifierReplacement::Replace(vec![
            second.clone(),
            first.clone()
        ]))
    );
    let current_single = [first.clone()];
    assert_eq!(
        plan_modifier_replacement(&current_single, &[first.clone(), first.clone()]),
        Err(CatalogError::DuplicateModifierGroup)
    );
    assert_eq!(
        plan_modifier_replacement(&[], &[]),
        Ok(ModifierReplacement::Unchanged)
    );
}

#[test]
fn revision_overflow_is_reported_instead_of_wrapping() {
    let mut category = Category::restore(
        id("00000000-0000-0000-0000-000000000280"),
        org(),
        code("cafeteria"),
        name("Cafetería"),
        Status::Active,
        u64::MAX,
    );
    assert_eq!(
        category.patch(CategoryPatch {
            expected_version: u64::MAX,
            name: Some(name("Otra")),
            status: None,
        }),
        Err(CatalogError::RevisionOverflow)
    );
    assert_eq!(category.name().as_str(), "Cafetería");
}

#[test]
fn restored_rows_keep_their_identity_and_reject_incoherent_selections() {
    let restored = Product::restore(
        id("00000000-0000-0000-0000-000000000290"),
        org(),
        Some(id("00000000-0000-0000-0000-000000000291")),
        sku("SKU-9"),
        Some(
            Barcode::new("750")
                .unwrap_or_else(|error| panic!("barcode: {error}"))
                .unwrap_or_else(|| panic!("barcode presente")),
        ),
        name("Producto"),
        None,
        price("9.99"),
        Status::Inactive,
        4,
    );
    assert_eq!(restored.sku().as_str(), "SKU-9");
    assert_eq!(restored.revision(), 4);
    assert!(!restored.is_active());

    assert_eq!(
        ModifierGroup::restore(
            id("00000000-0000-0000-0000-000000000292"),
            org(),
            code("size"),
            name("Tamaño"),
            2,
            1,
            Status::Active,
            0,
        ),
        Err(CatalogError::InvalidSelectionRange)
    );
}
