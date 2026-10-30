//! Pruebas de las reglas puras de proveedores.
#![allow(clippy::panic)]

use crate::{
    catalog::{CatalogCode, PatchField},
    organization_branch::{DisplayName, EntityId, Status},
    supplier::{
        BoundedText, MAX_LEGAL_NAME_UNITS, MAX_PHONE_UNITS, NewSupplier, Supplier, SupplierEmail,
        SupplierError, SupplierPatch, supplier_changed_fields,
    },
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn org() -> EntityId {
    id("00000000-0000-0000-0000-000000000801")
}
fn other_org() -> EntityId {
    id("00000000-0000-0000-0000-000000000802")
}
fn code(value: &str) -> CatalogCode {
    CatalogCode::new(value).unwrap_or_else(|error| panic!("fixture código: {error}"))
}
fn name(value: &str) -> DisplayName {
    DisplayName::new(value).unwrap_or_else(|error| panic!("fixture nombre: {error}"))
}
fn supplier() -> Supplier {
    Supplier::create(NewSupplier {
        id: id("00000000-0000-0000-0000-000000000810"),
        organization_id: org(),
        code: code("proveedor-uno"),
        name: name("Proveedor Uno"),
        legal_name: None,
        tax_id: None,
        contact_name: None,
        email: None,
        phone: None,
        address: None,
    })
}

#[test]
fn supplier_starts_active_with_its_organization_and_snapshot() {
    let supplier = supplier();
    assert_eq!(supplier.status(), Status::Active);
    assert!(supplier.is_active());
    assert_eq!(supplier.revision(), 0);
    assert_eq!(supplier.ensure_organization(&org()), Ok(()));
    assert_eq!(
        supplier.ensure_organization(&other_org()),
        Err(SupplierError::OwnershipMismatch)
    );
    let snapshot = supplier.snapshot();
    assert_eq!(snapshot.code.as_str(), "proveedor-uno");
    assert_eq!(snapshot.name.as_str(), "Proveedor Uno");
}

#[test]
fn optional_text_is_trimmed_bounded_and_blank_is_none() {
    assert_eq!(
        BoundedText::<MAX_PHONE_UNITS>::new("   ")
            .unwrap_or_else(|error| panic!("blanco: {error}")),
        None
    );
    assert_eq!(
        BoundedText::<MAX_PHONE_UNITS>::new(" 555-1234 ")
            .unwrap_or_else(|error| panic!("teléfono: {error}"))
            .map(|value| value.as_str().to_owned()),
        Some("555-1234".to_owned())
    );
    assert_eq!(
        BoundedText::<MAX_PHONE_UNITS>::new(&"1".repeat(41)),
        Err(SupplierError::TextTooLong)
    );
    assert!(
        BoundedText::<MAX_LEGAL_NAME_UNITS>::new(&"a".repeat(160)).is_ok(),
        "el límite exacto se admite"
    );
    assert_eq!(
        BoundedText::<MAX_PHONE_UNITS>::restore(" 555"),
        Err(SupplierError::InvalidPersistedValue)
    );
}

#[test]
fn email_is_lowercased_rejects_malformed_and_keeps_unicode_domains() {
    assert_eq!(
        SupplierEmail::new("  Contacto@Proveedor.TEST ")
            .unwrap_or_else(|error| panic!("email: {error}"))
            .map(|value| value.as_str().to_owned()),
        Some("contacto@proveedor.test".to_owned())
    );
    for invalid in [
        "",
        "sin-arroba.test",
        "@proveedor.test",
        "usuario@",
        "usuario@proveedor",
        "usuario@.test",
        "usuario@proveedor.",
        "us er@proveedor.test",
        "a@b@c.test",
        "usuario@proveedor .test",
    ] {
        let result = SupplierEmail::new(invalid);
        match invalid {
            "" => assert_eq!(result, Ok(None)),
            _ => assert_eq!(
                result,
                Err(SupplierError::InvalidEmail),
                "email aceptado indebidamente: {invalid}"
            ),
        }
    }
    assert_eq!(
        SupplierEmail::new("ñandú@café.example")
            .unwrap_or_else(|error| panic!("unicode: {error}"))
            .map(|value| value.as_str().to_owned()),
        Some("ñandú@café.example".to_owned()),
        "el plegado Unicode conserva la compatibilidad Java"
    );
}

#[test]
fn patch_updates_fields_atomically_and_reports_changes() {
    let mut supplier = supplier();
    let before = supplier.clone();

    assert_eq!(
        supplier.patch(SupplierPatch {
            expected_version: 0,
            name: Some(name("Proveedor Uno")),
            legal_name: PatchField::Absent,
            tax_id: PatchField::Absent,
            contact_name: PatchField::Absent,
            email: PatchField::Absent,
            phone: PatchField::Absent,
            address: PatchField::Absent,
            status: None,
        }),
        Ok(false),
        "un patch sin cambios no incrementa la revisión"
    );

    let email = SupplierEmail::new("Ventas@Proveedor.TEST")
        .unwrap_or_else(|error| panic!("email: {error}"))
        .unwrap_or_else(|| panic!("email presente"));
    let phone = BoundedText::<MAX_PHONE_UNITS>::new(" 555 ")
        .unwrap_or_else(|error| panic!("teléfono: {error}"))
        .unwrap_or_else(|| panic!("teléfono presente"));
    assert_eq!(
        supplier.patch(SupplierPatch {
            expected_version: 0,
            name: Some(name("Proveedor Uno SA")),
            legal_name: PatchField::Clear,
            tax_id: PatchField::Absent,
            contact_name: PatchField::Absent,
            email: PatchField::Set(email),
            phone: PatchField::Set(phone),
            address: PatchField::Absent,
            status: Some(Status::Inactive),
        }),
        Ok(true)
    );
    assert_eq!(supplier.name().as_str(), "Proveedor Uno SA");
    assert_eq!(supplier.email(), Some("ventas@proveedor.test"));
    assert_eq!(supplier.phone(), Some("555"));
    assert_eq!(supplier.status(), Status::Inactive);
    assert_eq!(supplier.revision(), 1);
    assert_eq!(
        supplier_changed_fields(&before, &supplier),
        vec!["name", "email", "phone", "status"]
    );
    assert_eq!(
        supplier.code().as_str(),
        "proveedor-uno",
        "el código es inmutable"
    );
}

#[test]
fn clearing_an_absent_optional_field_is_not_a_change() {
    let mut supplier = supplier();
    assert_eq!(
        supplier.patch(SupplierPatch {
            expected_version: 0,
            name: None,
            legal_name: PatchField::Clear,
            tax_id: PatchField::Clear,
            contact_name: PatchField::Clear,
            email: PatchField::Clear,
            phone: PatchField::Clear,
            address: PatchField::Clear,
            status: None,
        }),
        Ok(false)
    );
    assert_eq!(supplier.revision(), 0);
}

#[test]
fn patch_rejects_stale_versions_without_partial_edits() {
    let mut supplier = supplier();
    supplier
        .patch(SupplierPatch {
            expected_version: 0,
            name: Some(name("Proveedor Dos")),
            legal_name: PatchField::Absent,
            tax_id: PatchField::Absent,
            contact_name: PatchField::Absent,
            email: PatchField::Absent,
            phone: PatchField::Absent,
            address: PatchField::Absent,
            status: None,
        })
        .unwrap_or_else(|error| panic!("primer patch: {error}"));
    assert_eq!(
        supplier.patch(SupplierPatch {
            expected_version: 0,
            name: Some(name("Proveedor Tres")),
            legal_name: PatchField::Absent,
            tax_id: PatchField::Absent,
            contact_name: PatchField::Absent,
            email: PatchField::Absent,
            phone: PatchField::Absent,
            address: PatchField::Absent,
            status: Some(Status::Inactive),
        }),
        Err(SupplierError::ConcurrentModification)
    );
    assert_eq!(supplier.name().as_str(), "Proveedor Dos");
    assert_eq!(supplier.status(), Status::Active);
    assert_eq!(supplier.revision(), 1);
}

#[test]
fn revision_overflow_is_reported_instead_of_wrapping() {
    let mut supplier = Supplier::restore(
        id("00000000-0000-0000-0000-000000000811"),
        org(),
        code("proveedor-tope"),
        name("Proveedor Tope"),
        None,
        None,
        None,
        None,
        None,
        None,
        Status::Active,
        u64::MAX,
    );
    assert_eq!(
        supplier.patch(SupplierPatch {
            expected_version: u64::MAX,
            name: Some(name("Otro")),
            legal_name: PatchField::Absent,
            tax_id: PatchField::Absent,
            contact_name: PatchField::Absent,
            email: PatchField::Absent,
            phone: PatchField::Absent,
            address: PatchField::Absent,
            status: None,
        }),
        Err(SupplierError::RevisionOverflow)
    );
    assert_eq!(supplier.name().as_str(), "Proveedor Tope");
}

#[test]
fn restored_supplier_keeps_identity_and_optional_fields() {
    let legal = BoundedText::<MAX_LEGAL_NAME_UNITS>::new("Proveedor Uno SA de CV")
        .unwrap_or_else(|error| panic!("razón social: {error}"))
        .unwrap_or_else(|| panic!("razón social presente"));
    let supplier = Supplier::restore(
        id("00000000-0000-0000-0000-000000000812"),
        org(),
        code("proveedor-restaurado"),
        name("Proveedor Restaurado"),
        Some(legal),
        None,
        None,
        None,
        None,
        None,
        Status::Inactive,
        u64::MAX,
    );
    assert_eq!(supplier.legal_name(), Some("Proveedor Uno SA de CV"));
    assert_eq!(supplier.status(), Status::Inactive);
    assert!(!supplier.is_active());
    assert_eq!(supplier.revision(), u64::MAX);
}
