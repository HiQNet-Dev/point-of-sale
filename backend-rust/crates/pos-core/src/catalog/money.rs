use std::fmt;

use super::*;

/// Decimal exacto con escala fija 4, sin coma flotante.
///
/// Acepta un signo opcional, uno o más dígitos enteros y, si aparece el punto,
/// al menos un decimal. Rechaza exponentes, separadores, espacios y más de
/// cuatro decimales, incluidos los ceros finales: `1.50000` tiene escala 5 y el
/// agregado Java también lo rechaza.
///
/// Los dígitos enteros significativos no pueden superar
/// [`MAX_PRICE_INTEGER_DIGITS`]. La comparación se hace sobre la magnitud, de
/// modo que los ceros iniciales no alteran el resultado.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Decimal4(i128);

impl Decimal4 {
    /// Interpreta un decimal canónico ya expresado con hasta cuatro decimales.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si la sintaxis o el rango no
    /// corresponden al contrato.
    pub fn parse(input: &str) -> Result<Self, CatalogError> {
        let (negative, unsigned) = if let Some(rest) = input.strip_prefix('-') {
            (true, rest)
        } else if let Some(rest) = input.strip_prefix('+') {
            (false, rest)
        } else {
            (false, input)
        };
        let (integer, fraction) = match unsigned.split_once('.') {
            Some((integer, fraction)) => {
                if fraction.is_empty() {
                    return Err(CatalogError::InvalidPrice);
                }
                (integer, fraction)
            }
            None => (unsigned, ""),
        };
        if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(CatalogError::InvalidPrice);
        }
        if fraction.len() > PRICE_SCALE as usize
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(CatalogError::InvalidPrice);
        }
        let significant = integer.trim_start_matches('0');
        if significant.len() > MAX_PRICE_INTEGER_DIGITS {
            return Err(CatalogError::InvalidPrice);
        }
        let mut magnitude: i128 = 0;
        for byte in integer.bytes() {
            magnitude = magnitude * 10 + i128::from(byte - b'0');
        }
        magnitude *= MINOR_FACTOR;
        let mut scaled_fraction: i128 = 0;
        for byte in fraction.bytes() {
            scaled_fraction = scaled_fraction * 10 + i128::from(byte - b'0');
        }
        for _ in fraction.len()..PRICE_SCALE as usize {
            scaled_fraction *= 10;
        }
        magnitude += scaled_fraction;
        if magnitude >= MAX_MINOR_MAGNITUDE {
            return Err(CatalogError::InvalidPrice);
        }
        Ok(Self(if negative { -magnitude } else { magnitude }))
    }

    /// Construye desde unidades menores ya expresadas con escala 4.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si la magnitud excede el rango
    /// lógico de `NUMERIC(19,4)`.
    pub fn from_minor(minor: i128) -> Result<Self, CatalogError> {
        if minor.unsigned_abs() >= MAX_MINOR_MAGNITUDE.unsigned_abs() {
            return Err(CatalogError::InvalidPrice);
        }
        Ok(Self(minor))
    }

    /// Unidades menores con escala 4.
    pub fn as_minor(&self) -> i128 {
        self.0
    }

    /// Unidades menores si caben en el `INTEGER` de SQLite.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] cuando el valor lógico no cabe en
    /// un entero firmado de 64 bits; la representación física del catálogo
    /// Local sigue pendiente de aprobación.
    pub fn as_minor_i64(&self) -> Result<i64, CatalogError> {
        i64::try_from(self.0).map_err(|_| CatalogError::InvalidPrice)
    }

    /// Cero exacto con escala 4.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Indica si el valor es estrictamente negativo.
    pub fn is_negative(&self) -> bool {
        self.0 < 0
    }

    /// Indica si el valor es exactamente cero.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for Decimal4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let integer = magnitude / MINOR_FACTOR.unsigned_abs();
        let fraction = magnitude % MINOR_FACTOR.unsigned_abs();
        if self.0 < 0 {
            formatter.write_str("-")?;
        }
        write!(formatter, "{integer}.{fraction:04}")
    }
}

/// Precio base de producto: decimal exacto no negativo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitPrice(Decimal4);

impl UnitPrice {
    /// Acepta únicamente valores no negativos.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si el valor es negativo.
    pub fn new(value: Decimal4) -> Result<Self, CatalogError> {
        if value.is_negative() {
            return Err(CatalogError::InvalidPrice);
        }
        Ok(Self(value))
    }

    /// Interpreta el texto decimal y aplica la regla de no negatividad.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] en cualquier incumplimiento.
    pub fn parse(input: &str) -> Result<Self, CatalogError> {
        Self::new(Decimal4::parse(input)?)
    }

    /// Importe cero.
    pub fn zero() -> Self {
        Self(Decimal4::zero())
    }

    /// Valor decimal subyacente.
    pub fn value(&self) -> Decimal4 {
        self.0
    }
}

impl fmt::Display for UnitPrice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Delta de precio firmado de una opción de modificador.
///
/// Admite descuentos (negativo), sin cambio (cero) y recargos (positivo).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceDelta(Decimal4);

impl PriceDelta {
    /// Envuelve un decimal ya validado.
    pub fn new(value: Decimal4) -> Self {
        Self(value)
    }

    /// Interpreta el texto decimal con signo.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si la sintaxis o el rango fallan.
    pub fn parse(input: &str) -> Result<Self, CatalogError> {
        Ok(Self(Decimal4::parse(input)?))
    }

    /// Valor decimal subyacente.
    pub fn value(&self) -> Decimal4 {
        self.0
    }
}

impl fmt::Display for PriceDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
