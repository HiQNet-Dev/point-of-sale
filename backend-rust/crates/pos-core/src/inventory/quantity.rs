use std::fmt;

use super::*;

/// Unidad base de un artículo de inventario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryBaseUnit {
    /// Unidad discreta.
    Unit,
    /// Gramo.
    Gram,
    /// Mililitro.
    Milliliter,
}

impl InventoryBaseUnit {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unit => "UNIT",
            Self::Gram => "GRAM",
            Self::Milliliter => "MILLILITER",
        }
    }

    /// Convierte un valor persistido sin aceptar unidades desconocidas.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, InventoryError> {
        match value {
            "UNIT" => Ok(Self::Unit),
            "GRAM" => Ok(Self::Gram),
            "MILLILITER" => Ok(Self::Milliliter),
            _ => Err(InventoryError::InvalidPersistedValue),
        }
    }
}

/// Cantidad exacta con escala fija 6.
///
/// Sintaxis admitida: signo `-` opcional, uno o más dígitos enteros y, si
/// aparece el punto, al menos un decimal. Rechaza `+`, exponentes, espacios y
/// más de seis decimales, incluidos los ceros finales, igual que
/// `InventoryQuantity.parse`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Quantity(i128);

impl Quantity {
    /// Interpreta una cantidad decimal canónica.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidQuantity`] si la sintaxis o el rango no
    /// corresponden a `NUMERIC(19,6)`.
    pub fn parse(input: &str) -> Result<Self, InventoryError> {
        let (negative, unsigned) = match input.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, input),
        };
        let (integer, fraction) = match unsigned.split_once('.') {
            Some((integer, fraction)) => {
                if fraction.is_empty() {
                    return Err(InventoryError::InvalidQuantity);
                }
                (integer, fraction)
            }
            None => (unsigned, ""),
        };
        if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InventoryError::InvalidQuantity);
        }
        if fraction.len() > QUANTITY_SCALE as usize
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(InventoryError::InvalidQuantity);
        }
        let significant = integer.trim_start_matches('0');
        if significant.len() > MAX_QUANTITY_INTEGER_DIGITS {
            return Err(InventoryError::InvalidQuantity);
        }
        let mut magnitude = 0_i128;
        for byte in integer.bytes() {
            magnitude = magnitude * 10 + i128::from(byte - b'0');
        }
        magnitude *= MINOR_FACTOR;
        let mut scaled_fraction = 0_i128;
        for byte in fraction.bytes() {
            scaled_fraction = scaled_fraction * 10 + i128::from(byte - b'0');
        }
        for _ in fraction.len()..QUANTITY_SCALE as usize {
            scaled_fraction *= 10;
        }
        magnitude += scaled_fraction;
        if magnitude > MAX_MINOR_MAGNITUDE {
            return Err(InventoryError::InvalidQuantity);
        }
        Ok(Self(if negative { -magnitude } else { magnitude }))
    }

    /// Construye desde unidades menores ya expresadas con escala 6.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidQuantity`] fuera de rango.
    pub fn from_minor(minor: i128) -> Result<Self, InventoryError> {
        if minor.unsigned_abs() > MAX_MINOR_MAGNITUDE.unsigned_abs() {
            return Err(InventoryError::InvalidQuantity);
        }
        Ok(Self(minor))
    }

    /// Cantidad cero.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Unidades menores con escala 6.
    pub fn as_minor(&self) -> i128 {
        self.0
    }

    /// Unidades menores si caben en el `INTEGER` de SQLite.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidQuantity`] fuera del rango de `i64`.
    pub fn as_minor_i64(&self) -> Result<i64, InventoryError> {
        i64::try_from(self.0).map_err(|_| InventoryError::InvalidQuantity)
    }

    /// Indica si la cantidad es estrictamente negativa.
    pub fn is_negative(&self) -> bool {
        self.0 < 0
    }

    /// Indica si la cantidad es exactamente cero.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Indica si la cantidad tiene parte fraccionaria distinta de cero.
    pub fn is_fractional(&self) -> bool {
        self.0 % MINOR_FACTOR != 0
    }

    /// Suma exacta con verificación de rango.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::QuantityOverflow`].
    pub fn checked_add(&self, other: &Self) -> Result<Self, InventoryError> {
        let sum = self
            .0
            .checked_add(other.0)
            .ok_or(InventoryError::QuantityOverflow)?;
        Self::from_minor(sum).map_err(|_| InventoryError::QuantityOverflow)
    }

    /// Devuelve el opuesto exacto.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::QuantityOverflow`] si no es representable.
    pub fn checked_neg(&self) -> Result<Self, InventoryError> {
        let negated = self
            .0
            .checked_neg()
            .ok_or(InventoryError::QuantityOverflow)?;
        Self::from_minor(negated).map_err(|_| InventoryError::QuantityOverflow)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let integer = magnitude / MINOR_FACTOR.unsigned_abs();
        let fraction = magnitude % MINOR_FACTOR.unsigned_abs();
        if self.0 < 0 {
            formatter.write_str("-")?;
        }
        write!(formatter, "{integer}.{fraction:06}")
    }
}
