// Valida que la conversión entre familias monetarias sea exacta y ruidosa ante
// cualquier pérdida de precisión. No sustituye pruebas de persistencia.
#![allow(clippy::panic)]

use crate::catalog::Decimal4;
use crate::money_scale::*;
use crate::{Currency, MoneyMinor};

fn mxn() -> Currency {
    Currency::new("MXN").unwrap_or_else(|error| panic!("moneda: {error}"))
}

fn exponent(value: u8) -> MinorExponent {
    MinorExponent::new(value).unwrap_or_else(|error| panic!("exponente: {error}"))
}

fn decimal(value: &str) -> Decimal4 {
    Decimal4::parse(value).unwrap_or_else(|error| panic!("decimal: {error}"))
}

#[test]
fn exponent_rejects_scales_beyond_decimal4() {
    assert_eq!(exponent(0).get(), 0);
    assert_eq!(exponent(4).get(), 4);
    assert_eq!(
        MinorExponent::new(5),
        Err(MoneyScaleError::UnsupportedExponent)
    );
}

#[test]
fn minor_units_become_exact_scale4_for_each_exponent() {
    for (exponent_value, minor, expected) in [
        (0_u8, 1_000_i64, "1000.0000"),
        (2, 12_345, "123.4500"),
        (3, 999, "0.9990"),
        (4, 999, "0.0999"),
    ] {
        let converted = minor_to_decimal4(&MoneyMinor::new(minor, mxn()), exponent(exponent_value))
            .unwrap_or_else(|error| panic!("conversión: {error}"));
        assert_eq!(converted.to_string(), expected);
    }
}

#[test]
fn zero_is_zero_in_both_directions() {
    let zero = minor_to_decimal4(&MoneyMinor::new(0, mxn()), exponent(2))
        .unwrap_or_else(|error| panic!("conversión: {error}"));
    assert!(zero.is_zero());
    assert_eq!(
        decimal4_to_minor(zero, mxn(), exponent(2))
            .unwrap_or_else(|error| panic!("conversión: {error}"))
            .amount(),
        0
    );
}

#[test]
fn negative_minor_units_keep_their_sign() {
    let converted = minor_to_decimal4(&MoneyMinor::new(-1_500, mxn()), exponent(2))
        .unwrap_or_else(|error| panic!("conversión: {error}"));
    assert_eq!(converted.to_string(), "-15.0000");
    assert!(converted.is_negative());
    assert_eq!(
        decimal4_to_minor(converted, mxn(), exponent(2))
            .unwrap_or_else(|error| panic!("conversión: {error}"))
            .amount(),
        -1_500
    );
}

#[test]
fn round_trip_is_reversible_when_the_value_fits_the_currency() {
    for (exponent_value, minor) in [
        (0_u8, 7_i64),
        (2, 1_234_567),
        (3, -42),
        (4, i64::MAX / 10_000),
    ] {
        let original = MoneyMinor::new(minor, mxn());
        let decimal_value = minor_to_decimal4(&original, exponent(exponent_value))
            .unwrap_or_else(|error| panic!("ida: {error}"));
        let restored = decimal4_to_minor(decimal_value, mxn(), exponent(exponent_value))
            .unwrap_or_else(|error| panic!("vuelta: {error}"));
        assert_eq!(restored.amount(), original.amount());
        assert_eq!(restored.currency(), original.currency());
    }
}

#[test]
fn fractional_digits_below_the_currency_exponent_are_rejected() {
    assert_eq!(
        decimal4_to_minor(decimal("1.0050"), mxn(), exponent(2)),
        Err(MoneyScaleError::LossOfPrecision)
    );
    assert_eq!(
        decimal4_to_minor(decimal("0.0001"), mxn(), exponent(0)),
        Err(MoneyScaleError::LossOfPrecision)
    );
    // El mismo valor sí es representable cuando la moneda usa cuatro decimales.
    assert_eq!(
        decimal4_to_minor(decimal("1.0050"), mxn(), exponent(4))
            .unwrap_or_else(|error| panic!("conversión: {error}"))
            .amount(),
        10_050
    );
}

#[test]
fn values_outside_the_scale4_range_are_not_silently_truncated() {
    // 9,22e18 unidades menores con dos decimales superan NUMERIC(19,4).
    assert_eq!(
        minor_to_decimal4(&MoneyMinor::new(i64::MAX, mxn()), exponent(2)),
        Err(MoneyScaleError::OutOfRange)
    );
}

#[test]
fn values_beyond_i64_units_are_reported_as_overflow() {
    // 999999999999999.9999 * 10^4 = 9,99e18 > i64::MAX.
    assert_eq!(
        decimal4_to_minor(decimal("999999999999999.9999"), mxn(), exponent(4)),
        Err(MoneyScaleError::Overflow)
    );
    // El mismo importe con dos decimales cabe y se convierte exactamente.
    assert_eq!(
        decimal4_to_minor(decimal("999999999999999.9900"), mxn(), exponent(2))
            .unwrap_or_else(|error| panic!("conversión: {error}"))
            .amount(),
        99_999_999_999_999_999
    );
}

#[test]
fn currency_of_the_result_is_the_requested_one() {
    let converted = decimal4_to_minor(decimal("10.0000"), mxn(), exponent(2))
        .unwrap_or_else(|error| panic!("conversión: {error}"));
    assert_eq!(converted.currency().as_str(), "MXN");
}
