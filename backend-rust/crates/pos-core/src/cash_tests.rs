//! Pruebas de las reglas puras de cajas y turnos.
#![allow(clippy::panic)]

use crate::{
    cash::{
        CashError, CashRegister, CashRegisterPatch, CashShift, CashShiftStatus, NewCashRegister,
        NewCashShift, cash_register_changed_fields, ensure_close_allowed, ensure_open_allowed,
    },
    catalog::{CatalogCode, UnitPrice},
    identity::UtcSeconds,
    organization_branch::{DisplayName, EntityId, Status},
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn org() -> EntityId {
    id("00000000-0000-0000-0000-000000000301")
}
fn branch() -> EntityId {
    id("00000000-0000-0000-0000-000000000302")
}
fn other_branch() -> EntityId {
    id("00000000-0000-0000-0000-000000000303")
}
fn operator() -> EntityId {
    id("00000000-0000-0000-0000-000000000304")
}
fn supervisor() -> EntityId {
    id("00000000-0000-0000-0000-000000000305")
}
fn name(value: &str) -> DisplayName {
    DisplayName::new(value).unwrap_or_else(|error| panic!("fixture nombre: {error}"))
}
fn code(value: &str) -> CatalogCode {
    CatalogCode::new(value).unwrap_or_else(|error| panic!("fixture código: {error}"))
}
fn amount(value: &str) -> UnitPrice {
    UnitPrice::parse(value).unwrap_or_else(|error| panic!("fixture importe: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture instante: {error}"))
}
fn register() -> CashRegister {
    CashRegister::create(NewCashRegister {
        id: id("00000000-0000-0000-0000-000000000310"),
        organization_id: org(),
        branch_id: branch(),
        code: code("caja-1"),
        name: name("Caja 1"),
    })
}
fn shift(register: CashRegister) -> CashShift {
    CashShift::open(NewCashShift {
        id: id("00000000-0000-0000-0000-000000000320"),
        register,
        opened_by_user_id: operator(),
        opening_amount: amount("500"),
        opened_at: moment(1_800_000_000),
    })
    .unwrap_or_else(|error| panic!("fixture turno: {error}"))
}

#[test]
fn cash_register_patch_is_atomic_and_scope_is_immutable() {
    let mut register = register();
    assert_eq!(register.revision(), 0);
    assert!(register.is_active());
    assert_eq!(
        register.ensure_scope(&org(), &branch()),
        Ok(()),
        "la caja pertenece a su organización y sucursal"
    );
    assert_eq!(
        register.ensure_scope(&org(), &other_branch()),
        Err(CashError::OwnershipMismatch)
    );

    assert_eq!(
        register.patch(CashRegisterPatch {
            expected_version: 0,
            name: Some(name("Caja 1")),
            status: None,
        }),
        Ok(false),
        "un patch sin cambios no incrementa la revisión"
    );

    let before = register.clone();
    assert_eq!(
        register.patch(CashRegisterPatch {
            expected_version: 0,
            name: Some(name("  Caja principal  ")),
            status: Some(Status::Inactive),
        }),
        Ok(true)
    );
    assert_eq!(register.name().as_str(), "Caja principal");
    assert_eq!(register.revision(), 1);
    assert!(!register.is_active());
    assert_eq!(
        cash_register_changed_fields(&before, &register),
        vec!["name", "status"]
    );
    assert_eq!(register.branch_id(), &branch(), "la sucursal no cambia");
    assert_eq!(register.code().as_str(), "caja-1", "el código no cambia");
}

#[test]
fn cash_register_rejects_stale_versions_and_revision_overflow() {
    let mut register = register();
    register
        .patch(CashRegisterPatch {
            expected_version: 0,
            name: Some(name("Caja 2")),
            status: None,
        })
        .unwrap_or_else(|error| panic!("patch: {error}"));
    assert_eq!(
        register.patch(CashRegisterPatch {
            expected_version: 0,
            name: Some(name("Caja 3")),
            status: None,
        }),
        Err(CashError::ConcurrentModification)
    );
    assert_eq!(register.name().as_str(), "Caja 2");

    let mut exhausted = CashRegister::restore(
        id("00000000-0000-0000-0000-000000000311"),
        org(),
        branch(),
        code("caja-9"),
        name("Caja 9"),
        Status::Active,
        u64::MAX,
    );
    assert_eq!(
        exhausted.patch(CashRegisterPatch {
            expected_version: u64::MAX,
            name: Some(name("Otra")),
            status: None,
        }),
        Err(CashError::RevisionOverflow)
    );
}

#[test]
fn open_shift_derives_its_scope_from_the_register() {
    let register = register();
    let shift = shift(register.clone());
    assert_eq!(shift.organization_id(), register.organization_id());
    assert_eq!(shift.branch_id(), register.branch_id());
    assert_eq!(shift.cash_register_id(), register.id());
    assert_eq!(shift.status(), CashShiftStatus::Open);
    assert!(shift.is_open());
    assert_eq!(shift.opening_amount().to_string(), "500.0000");
    assert_eq!(shift.revision(), 0);
    assert_eq!(shift.closed_by_user_id(), None);
    assert_eq!(shift.closed_at(), None);
    assert_eq!(shift.ensure_scope(&org(), &branch(), register.id()), Ok(()));
    assert_eq!(
        shift.ensure_scope(&org(), &other_branch(), register.id()),
        Err(CashError::OwnershipMismatch)
    );
}

#[test]
fn opening_requires_an_active_register() {
    let mut register = register();
    register
        .patch(CashRegisterPatch {
            expected_version: 0,
            name: None,
            status: Some(Status::Inactive),
        })
        .unwrap_or_else(|error| panic!("desactivar: {error}"));
    assert_eq!(
        CashShift::open(NewCashShift {
            id: id("00000000-0000-0000-0000-000000000321"),
            register,
            opened_by_user_id: operator(),
            opening_amount: amount("0"),
            opened_at: moment(1_800_000_000),
        }),
        Err(CashError::RegisterInactive)
    );
}

#[test]
fn open_policy_matches_the_java_service_checks() {
    let register = register();
    assert_eq!(ensure_open_allowed(true, &register, false), Ok(()));
    assert_eq!(
        ensure_open_allowed(false, &register, false),
        Err(CashError::BranchInactive)
    );
    assert_eq!(
        ensure_open_allowed(true, &register, true),
        Err(CashError::RegisterAlreadyHasOpenShift)
    );

    let mut inactive = register.clone();
    inactive
        .patch(CashRegisterPatch {
            expected_version: 0,
            name: None,
            status: Some(Status::Inactive),
        })
        .unwrap_or_else(|error| panic!("desactivar: {error}"));
    assert_eq!(
        ensure_open_allowed(true, &inactive, false),
        Err(CashError::RegisterInactive)
    );
    assert_eq!(
        ensure_open_allowed(false, &inactive, true),
        Err(CashError::BranchInactive),
        "el orden de comprobación de Java se conserva"
    );
}

#[test]
fn closing_records_the_declaration_and_bumps_the_revision() {
    let mut shift = shift(register());
    let closed_at = moment(1_800_003_600);
    assert_eq!(
        shift.close(crate::cash::CloseCashShift {
            expected_version: 0,
            declared_amount: amount("480.5"),
            closed_by_user_id: supervisor(),
            closed_at,
        }),
        Ok(())
    );
    assert_eq!(shift.status(), CashShiftStatus::Closed);
    assert!(!shift.is_open());
    assert_eq!(shift.revision(), 1);
    assert_eq!(shift.closed_by_user_id(), Some(&supervisor()));
    assert_eq!(shift.closed_at(), Some(closed_at));
    assert_eq!(
        shift
            .closing_declared_amount()
            .map(|value| value.to_string()),
        Some("480.5000".to_owned())
    );
    assert_eq!(
        shift.opening_amount().to_string(),
        "500.0000",
        "la apertura no se altera al cerrar"
    );
}

#[test]
fn closing_twice_is_an_error_and_never_a_no_op() {
    let mut shift = shift(register());
    let request = crate::cash::CloseCashShift {
        expected_version: 0,
        declared_amount: amount("500"),
        closed_by_user_id: supervisor(),
        closed_at: moment(1_800_003_600),
    };
    shift
        .close(request.clone())
        .unwrap_or_else(|error| panic!("primer cierre: {error}"));
    let closed_state = shift.clone();
    assert_eq!(shift.close(request), Err(CashError::ShiftAlreadyClosed));
    assert_eq!(
        shift, closed_state,
        "un cierre rechazado no modifica el histórico"
    );
}

#[test]
fn closing_guards_version_and_open_sales() {
    let mut shift = shift(register());
    assert_eq!(
        shift.close(crate::cash::CloseCashShift {
            expected_version: 1,
            declared_amount: amount("500"),
            closed_by_user_id: supervisor(),
            closed_at: moment(1_800_003_600),
        }),
        Err(CashError::ConcurrentModification)
    );
    assert!(shift.is_open());
    assert_eq!(shift.revision(), 0);

    assert_eq!(ensure_close_allowed(&shift, false), Ok(()));
    assert_eq!(
        ensure_close_allowed(&shift, true),
        Err(CashError::ShiftHasOpenSales)
    );
}

#[test]
fn closing_does_not_require_an_active_branch_or_register() {
    // El turno se abrió cuando el recurso estaba activo; después se desactiva.
    let register = register();
    let mut shift = shift(register.clone());
    let mut deactivated = register;
    deactivated
        .patch(CashRegisterPatch {
            expected_version: 0,
            name: None,
            status: Some(Status::Inactive),
        })
        .unwrap_or_else(|error| panic!("desactivar: {error}"));
    assert!(!deactivated.is_active());
    assert_eq!(
        ensure_close_allowed(&shift, false),
        Ok(()),
        "el cierre recupera turnos de recursos ya desactivados"
    );
    assert_eq!(
        shift.close(crate::cash::CloseCashShift {
            expected_version: 0,
            declared_amount: amount("0"),
            closed_by_user_id: supervisor(),
            closed_at: moment(1_800_003_600),
        }),
        Ok(())
    );
}

#[test]
fn restored_shifts_require_coherent_state() {
    let common = || {
        (
            id("00000000-0000-0000-0000-000000000330"),
            org(),
            branch(),
            id("00000000-0000-0000-0000-000000000310"),
            operator(),
            amount("100"),
            moment(1_800_000_000),
        )
    };
    let (id_value, organization, branch_value, register_id, opener, opening, opened_at) = common();

    assert_eq!(
        CashShift::restore(
            id_value.clone(),
            organization.clone(),
            branch_value.clone(),
            register_id.clone(),
            opener.clone(),
            None,
            CashShiftStatus::Open,
            opening,
            None,
            opened_at,
            None,
            0,
        )
        .map(|shift| shift.is_open()),
        Ok(true)
    );
    assert_eq!(
        CashShift::restore(
            id_value.clone(),
            organization.clone(),
            branch_value.clone(),
            register_id.clone(),
            opener.clone(),
            Some(supervisor()),
            CashShiftStatus::Open,
            opening,
            Some(amount("1")),
            opened_at,
            None,
            1,
        ),
        Err(CashError::InvalidPersistedValue),
        "un turno abierto no puede tener datos de cierre"
    );
    assert_eq!(
        CashShift::restore(
            id_value,
            organization,
            branch_value,
            register_id,
            opener,
            None,
            CashShiftStatus::Closed,
            opening,
            None,
            opened_at,
            None,
            1,
        ),
        Err(CashError::InvalidPersistedValue),
        "un turno cerrado exige todos los datos de cierre"
    );
    assert_eq!(CashShiftStatus::parse("OPEN"), Ok(CashShiftStatus::Open));
    assert_eq!(
        CashShiftStatus::parse("REOPENED"),
        Err(CashError::InvalidPersistedValue)
    );
}
