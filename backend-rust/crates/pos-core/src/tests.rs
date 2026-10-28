use super::{Currency, MoneyError, MoneyMinor};

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
