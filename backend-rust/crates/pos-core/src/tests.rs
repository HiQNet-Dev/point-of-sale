use super::{
    Branch, BranchId, BranchPatch, BranchStatus, BusinessCode, Currency, DisplayName, DomainError,
    IanaTimezone, InventoryNegativeStockPolicy, LocaleTag, MoneyError, MoneyMinor, Organization,
    OrganizationCurrency, OrganizationId, OrganizationPatch, OrganizationStatus, Quantity,
    QuantityError, Revision, TimezonePatch,
};
use uuid::Uuid;

#[test]
fn currency_normalizes_ascii_letters_without_claiming_iso_membership() -> Result<(), MoneyError> {
    assert_eq!(Currency::new("uSd")?.as_str(), "USD");
    assert_eq!(Currency::new("zzz")?.as_str(), "ZZZ");
    Ok(())
}

#[test]
fn currency_rejects_non_ascii_or_wrong_length() -> Result<(), MoneyError> {
    for value in [
        "",
        "US",
        "USDD",
        "U1D",
        " US",
        "USD ",
        "€UR",
        "ábc",
        "ＵＳＤ",
    ] {
        assert_eq!(Currency::new(value), Err(MoneyError::InvalidCurrency));
    }
    Ok(())
}

#[test]
fn signed_minor_units_preserve_currency_and_add_exactly() -> Result<(), MoneyError> {
    let currency = Currency::new("USD")?;
    let amount = MoneyMinor::new(1_005, currency.clone());
    let refund = MoneyMinor::new(-1_006, currency.clone());
    let sum = amount.checked_add(&refund)?;
    assert_eq!(sum.amount(), -1);
    assert_eq!(sum.currency(), &currency);
    assert_eq!(amount.amount(), 1_005);
    Ok(())
}

#[test]
fn addition_rejects_currency_mismatch_and_both_overflow_directions() -> Result<(), MoneyError> {
    let usd = Currency::new("USD")?;
    let eur = Currency::new("EUR")?;
    assert_eq!(
        MoneyMinor::new(1, usd.clone()).checked_add(&MoneyMinor::new(1, eur)),
        Err(MoneyError::CurrencyMismatch)
    );
    for (amount, increment) in [(i64::MAX, 1), (i64::MIN, -1)] {
        assert_eq!(
            MoneyMinor::new(amount, usd.clone())
                .checked_add(&MoneyMinor::new(increment, usd.clone())),
            Err(MoneyError::Overflow)
        );
    }
    Ok(())
}

#[test]
fn decimal_import_is_exact_with_explicit_scale_and_no_rounding() -> Result<(), MoneyError> {
    let currency = Currency::new("USD")?;
    for (input, scale, expected) in [
        ("0.1", 4, 1_000),
        ("123456789012345.6789", 4, 1_234_567_890_123_456_789),
        ("-123456789012345.6789", 4, -1_234_567_890_123_456_789),
        ("+001.23000", 2, 123),
        ("-0.0000", 4, 0),
        ("42", 2, 4_200),
        ("42.000", 0, 42),
        ("0.000000000000000001", 18, 1),
        ("1", 18, 1_000_000_000_000_000_000),
    ] {
        assert_eq!(
            MoneyMinor::from_decimal_str(input, currency.clone(), scale)?.amount(),
            expected,
            "entrada {input}, escala {scale}"
        );
    }
    for (input, scale) in [("1.001", 2), ("-0.00001", 4), ("1.1", 0)] {
        assert_eq!(
            MoneyMinor::from_decimal_str(input, currency.clone(), scale),
            Err(MoneyError::NonRepresentableFraction)
        );
    }
    Ok(())
}

#[test]
fn decimal_19_4_import_accepts_i64_boundaries_and_rejects_overflow() -> Result<(), MoneyError> {
    let currency = Currency::new("USD")?;
    for (input, expected) in [
        ("922337203685477.5807", i64::MAX),
        ("-922337203685477.5808", i64::MIN),
    ] {
        assert_eq!(
            MoneyMinor::from_decimal_str(input, currency.clone(), 4)?.amount(),
            expected
        );
    }
    for input in [
        "922337203685477.5808",
        "-922337203685477.5809",
        "999999999999999.9999",
        "-999999999999999.9999",
        "99999999999999999999999999999999999999999",
    ] {
        assert_eq!(
            MoneyMinor::from_decimal_str(input, currency.clone(), 4),
            Err(MoneyError::Overflow)
        );
    }
    Ok(())
}

#[test]
fn decimal_import_rejects_invalid_scales_and_non_decimal_syntax() -> Result<(), MoneyError> {
    let currency = Currency::new("USD")?;
    for scale in [19, 20, u32::MAX] {
        assert_eq!(
            MoneyMinor::from_decimal_str("0", currency.clone(), scale),
            Err(MoneyError::InvalidScale)
        );
    }
    for input in [
        "", "+", "-", ".1", "1.", ".", "1.2.3", " 1", "1 ", "1\n", "1e2", "1,23", "NaN", "inf",
        "١", "−1", "1_000", "--1", "+-1",
    ] {
        assert_eq!(
            MoneyMinor::from_decimal_str(input, currency.clone(), 4),
            Err(MoneyError::InvalidDecimal),
            "entrada {input:?}"
        );
    }
    Ok(())
}

#[test]
fn decimal_import_handles_leading_zeroes_without_intermediate_overflow() -> Result<(), MoneyError> {
    let currency = Currency::new("USD")?;
    assert_eq!(
        MoneyMinor::from_decimal_str(
            "00000000000000000000000000000000000000001.2300000000000000000000000",
            currency,
            2,
        )?
        .amount(),
        123
    );
    Ok(())
}

#[test]
fn quantity_preserves_exact_six_decimal_values_and_canonicalizes_output()
-> Result<(), QuantityError> {
    for (input, micros, canonical) in [
        ("0", 0, "0.000000"),
        ("+001.2500000", 1_250_000, "1.250000"),
        ("-0.000001", -1, "-0.000001"),
        (
            "9999999999999.999999",
            9_999_999_999_999_999_999,
            "9999999999999.999999",
        ),
    ] {
        let quantity = Quantity::from_decimal_str(input)?;
        assert_eq!(quantity.micros(), micros);
        assert_eq!(quantity.to_canonical_string(), canonical);
    }
    Ok(())
}

#[test]
fn quantity_rejects_lossy_values_invalid_syntax_and_out_of_range_values() {
    for input in ["1.0000001", "-0.0000001"] {
        assert_eq!(
            Quantity::from_decimal_str(input),
            Err(QuantityError::NonRepresentableFraction)
        );
    }
    for input in ["", "+", "1.", ".1", "1e2", "1,0", " 1", "١", "--1"] {
        assert_eq!(
            Quantity::from_decimal_str(input),
            Err(QuantityError::InvalidDecimal),
            "entrada {input:?}"
        );
    }
    for input in ["10000000000000", "-10000000000000"] {
        assert_eq!(
            Quantity::from_decimal_str(input),
            Err(QuantityError::OutOfRange)
        );
    }
}

#[test]
fn quantity_operations_are_checked_and_do_not_treat_zero_as_absent() -> Result<(), QuantityError> {
    let one = Quantity::from_decimal_str("1")?;
    let fraction = Quantity::from_decimal_str("0.000001")?;
    let zero = Quantity::from_decimal_str("0")?;
    assert_eq!(one.checked_sub(fraction)?.to_canonical_string(), "0.999999");
    assert_eq!(
        zero.checked_add(fraction)?.to_canonical_string(),
        "0.000001"
    );
    assert!(zero.is_zero());
    assert!(Quantity::from_decimal_str("-0.000001")?.is_negative());
    assert_eq!(
        Quantity::from_micros(Quantity::MAX_MICROS)?.checked_add(fraction),
        Err(QuantityError::OutOfRange)
    );
    Ok(())
}

fn organization() -> Result<Organization, DomainError> {
    Ok(Organization::create(
        OrganizationId::new(Uuid::nil()),
        DisplayName::parse("  Cafetería Central  ")?,
        BusinessCode::parse("cafeteria-central")?,
        IanaTimezone::parse("America/Mexico_City")?,
        OrganizationCurrency::parse("mxn")?,
        LocaleTag::parse("es-MX")?,
    ))
}

#[test]
fn organization_creation_preserves_defaults_identity_and_validated_metadata()
-> Result<(), DomainError> {
    let organization = organization()?;
    assert_eq!(organization.id().as_uuid(), Uuid::nil());
    assert_eq!(organization.name().as_str(), "Cafetería Central");
    assert_eq!(organization.code().as_str(), "cafeteria-central");
    assert_eq!(organization.currency().as_str(), "MXN");
    assert_eq!(organization.status(), OrganizationStatus::Active);
    assert_eq!(
        organization.inventory_negative_stock_policy(),
        InventoryNegativeStockPolicy::Strict
    );
    assert_eq!(organization.revision(), Revision::INITIAL);
    assert!(organization.is_active());
    Ok(())
}

#[test]
fn organization_rejects_invalid_codes_timezones_currencies_and_locales() {
    for code in [
        "",
        "Cafe",
        "cafe--central",
        "cafe central",
        "c".repeat(65).as_str(),
    ] {
        assert_eq!(BusinessCode::parse(code), Err(DomainError::InvalidCode));
    }
    for timezone in ["", "Not/AZone", "x".repeat(65).as_str()] {
        assert_eq!(
            IanaTimezone::parse(timezone),
            Err(DomainError::InvalidTimezone)
        );
    }
    for currency in ["", "US", "USDD", "XYZ", "12$"] {
        assert_eq!(
            OrganizationCurrency::parse(currency),
            Err(DomainError::InvalidCurrency)
        );
    }
    for locale in ["", "not_a_locale", "x".repeat(21).as_str()] {
        assert_eq!(LocaleTag::parse(locale), Err(DomainError::InvalidLocale));
    }
}

#[test]
fn organization_patch_updates_only_material_fields_and_enforces_revision() -> Result<(), DomainError>
{
    let mut organization = organization()?;
    let unchanged = organization.apply_patch(Revision::INITIAL, OrganizationPatch::default())?;
    assert!(unchanged.is_empty());
    assert_eq!(organization.revision(), Revision::INITIAL);
    let changed = organization.apply_patch(
        Revision::INITIAL,
        OrganizationPatch {
            name: Some(DisplayName::parse("Café Renovado")?),
            status: Some(OrganizationStatus::Inactive),
            inventory_negative_stock_policy: Some(InventoryNegativeStockPolicy::AllowNegative),
            ..OrganizationPatch::default()
        },
    )?;
    assert_eq!(
        changed.as_slice(),
        ["name", "status", "inventoryNegativeStockPolicy"]
    );
    assert_eq!(organization.revision().value(), 1);
    assert!(!organization.is_active());
    assert_eq!(
        organization.apply_patch(Revision::INITIAL, OrganizationPatch::default()),
        Err(DomainError::VersionMismatch)
    );
    Ok(())
}

#[test]
fn branch_inherits_timezone_and_cannot_change_owner_or_code() -> Result<(), DomainError> {
    let organization = organization()?;
    let inherited = organization.timezone().clone();
    let mut branch = Branch::create(
        BranchId::new(Uuid::from_u128(1)),
        organization.id(),
        DisplayName::parse("  Sucursal Centro ")?,
        BusinessCode::parse("centro")?,
        None,
    );
    assert_eq!(branch.organization_id(), organization.id());
    assert_eq!(branch.code().as_str(), "centro");
    assert_eq!(
        branch.effective_timezone(&inherited).as_str(),
        "America/Mexico_City"
    );
    assert_eq!(branch.status(), BranchStatus::Active);
    let no_change = branch.apply_patch(Revision::INITIAL, BranchPatch::default())?;
    assert!(no_change.is_empty());
    let override_timezone = IanaTimezone::parse("America/Argentina/Buenos_Aires")?;
    let changed = branch.apply_patch(
        Revision::INITIAL,
        BranchPatch {
            timezone: TimezonePatch::Set(override_timezone.clone()),
            status: Some(BranchStatus::Inactive),
            ..BranchPatch::default()
        },
    )?;
    assert_eq!(changed.as_slice(), ["timezone", "status"]);
    assert_eq!(branch.effective_timezone(&inherited), &override_timezone);
    assert_eq!(branch.revision().value(), 1);
    let cleared = branch.apply_patch(
        Revision::new(1),
        BranchPatch {
            timezone: TimezonePatch::ClearToInherited,
            ..BranchPatch::default()
        },
    )?;
    assert_eq!(cleared.as_slice(), ["timezone"]);
    assert_eq!(branch.effective_timezone(&inherited), &inherited);
    assert_eq!(
        branch.apply_patch(Revision::INITIAL, BranchPatch::default()),
        Err(DomainError::VersionMismatch)
    );
    Ok(())
}
