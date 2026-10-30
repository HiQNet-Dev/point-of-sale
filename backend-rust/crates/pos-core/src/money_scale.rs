//! Conversión explícita entre las dos familias monetarias del workspace.
//!
//! `pos-core` conserva hoy dos representaciones deliberadamente distintas:
//!
//! | Familia | Tipo | Escala | Rango |
//! | --- | --- | --- | --- |
//! | Unidades menores | [`crate::MoneyMinor`] | la de la moneda | `i64` |
//! | Decimal exacto | [`crate::catalog::Decimal4`] | 4 fija (`NUMERIC(19,4)`) | `i128` |
//!
//! Ninguna de las dos es convertible en la otra sin conocer cuántos decimales
//! usa la moneda, y ninguna conversión puede ser implícita: un redondeo
//! silencioso en un precio o en un total es un defecto financiero. Este módulo
//! expone la única vía autorizada, exige el exponente de la moneda como entrada
//! y falla con un error tipado cuando el valor no es representable sin pérdida.
//!
//! # Límite conocido
//!
//! `Decimal4` admite magnitudes `NUMERIC(19,4)` que no caben en un `INTEGER` de
//! SQLite ni en `i64`. La conversión a unidades menores rechaza esos valores con
//! [`MoneyScaleError::Overflow`] en lugar de truncarlos.

use std::fmt;

use crate::catalog::{CatalogError, Decimal4, PRICE_SCALE};
use crate::{Currency, MoneyMinor};

/// Exponente decimal máximo que `Decimal4` puede representar sin pérdida.
pub const MAX_MINOR_EXPONENT: u8 = PRICE_SCALE as u8;

/// Cantidad de decimales de una moneda, de `0` a [`MAX_MINOR_EXPONENT`].
///
/// El valor no se deduce del código ISO: `pos-core` no incorpora la tabla de
/// monedas y una suposición por longitud de código sería incorrecta (por ejemplo
/// `JPY` no usa dos decimales).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MinorExponent(u8);

impl MinorExponent {
    /// Acepta exponentes representables por [`Decimal4`].
    ///
    /// # Errors
    /// Devuelve [`MoneyScaleError::UnsupportedExponent`] si el exponente supera
    /// [`MAX_MINOR_EXPONENT`].
    pub const fn new(exponent: u8) -> Result<Self, MoneyScaleError> {
        if exponent > MAX_MINOR_EXPONENT {
            return Err(MoneyScaleError::UnsupportedExponent);
        }
        Ok(Self(exponent))
    }

    /// Exponente decimal de la moneda.
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Fallos de conversión, sin conservar importes ni datos de entrada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoneyScaleError {
    /// El exponente de la moneda excede la escala de `Decimal4`.
    UnsupportedExponent,
    /// El valor escala 4 queda fuera del rango lógico `NUMERIC(19,4)`.
    OutOfRange,
    /// El valor escala 4 tiene decimales que las unidades menores no admiten.
    LossOfPrecision,
    /// El resultado no cabe en el entero firmado de 64 bits de las unidades menores.
    Overflow,
}

impl fmt::Display for MoneyScaleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedExponent => "la moneda admite más decimales que la escala 4",
            Self::OutOfRange => "el importe excede el rango de la escala 4",
            Self::LossOfPrecision => "el importe no es representable sin pérdida de precisión",
            Self::Overflow => "el importe excede el rango de i64",
        })
    }
}

impl std::error::Error for MoneyScaleError {}

impl From<CatalogError> for MoneyScaleError {
    fn from(_: CatalogError) -> Self {
        Self::OutOfRange
    }
}

/// Convierte unidades menores a un decimal exacto de escala 4.
///
/// La operación es siempre exacta: multiplicar por `10^(4 - exponente)` no
/// introduce fracciones nuevas. Falla solo si el resultado se sale del rango
/// lógico de `NUMERIC(19,4)`.
///
/// # Errors
/// Devuelve [`MoneyScaleError::OutOfRange`] o [`MoneyScaleError::Overflow`].
pub fn minor_to_decimal4(
    amount: &MoneyMinor,
    exponent: MinorExponent,
) -> Result<Decimal4, MoneyScaleError> {
    let factor = 10_i128.pow(u32::from(MAX_MINOR_EXPONENT - exponent.get()));
    let scaled = i128::from(amount.amount())
        .checked_mul(factor)
        .ok_or(MoneyScaleError::Overflow)?;
    Decimal4::from_minor(scaled).map_err(MoneyScaleError::from)
}

/// Convierte un decimal exacto de escala 4 a unidades menores de una moneda.
///
/// Exige que los dígitos decimales por debajo del exponente de la moneda sean
/// cero. Un valor como `1.0050` con exponente `2` se rechaza con
/// [`MoneyScaleError::LossOfPrecision`] en lugar de redondearse.
///
/// # Errors
/// Devuelve [`MoneyScaleError::LossOfPrecision`] o [`MoneyScaleError::Overflow`].
pub fn decimal4_to_minor(
    value: Decimal4,
    currency: Currency,
    exponent: MinorExponent,
) -> Result<MoneyMinor, MoneyScaleError> {
    let divisor = 10_i128.pow(u32::from(MAX_MINOR_EXPONENT - exponent.get()));
    let minor = value.as_minor();
    if minor % divisor != 0 {
        return Err(MoneyScaleError::LossOfPrecision);
    }
    let minor = i64::try_from(minor / divisor).map_err(|_| MoneyScaleError::Overflow)?;
    Ok(MoneyMinor::new(minor, currency))
}
