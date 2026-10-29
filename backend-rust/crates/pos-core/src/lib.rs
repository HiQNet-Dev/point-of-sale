//! Primitivas monetarias exactas, sin dependencias de infraestructura.
//!
//! Las unidades menores no incorporan una escala implícita ni políticas de
//! redondeo. La importación decimal exige que el llamador proporcione la escala.

use std::fmt;

/// Foundation pura de auditoría Local, independiente de DDL y adaptadores.
pub mod audit;
/// Dominio Organization/Branch y puertos de aplicación reconstruidos tras INC-RECOVERY-01.
pub mod organization_branch;

/// Código de moneda de tres letras ASCII, normalizado a mayúsculas.
///
/// Solo valida la sintaxis; no certifica pertenencia al catálogo ISO 4217
/// ni conoce la cantidad de decimales de una moneda.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Currency(String);

impl Currency {
    /// Valida tres letras ASCII y normaliza su representación a mayúsculas.
    ///
    /// # Errors
    /// Devuelve [`MoneyError::InvalidCurrency`] para cualquier otra sintaxis.
    pub fn new(code: &str) -> Result<Self, MoneyError> {
        if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(MoneyError::InvalidCurrency);
        }
        Ok(Self(code.to_ascii_uppercase()))
    }

    /// Devuelve el código normalizado de tres letras ASCII.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Importe firmado en unidades menores de una moneda.
///
/// Admite saldos negativos y devoluciones. No realiza conversiones entre monedas
/// ni conserva una escala decimal; quien importa datos debe establecerla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoneyMinor {
    amount: i64,
    currency: Currency,
}

impl MoneyMinor {
    /// Construye un importe ya expresado en unidades menores, sin conversión.
    pub fn new(amount: i64, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Devuelve las unidades menores firmadas.
    pub fn amount(&self) -> i64 {
        self.amount
    }

    /// Devuelve la moneda del importe.
    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Suma importes de la misma moneda sin saturar ni desbordar.
    ///
    /// # Errors
    /// Devuelve [`MoneyError::CurrencyMismatch`] si las monedas difieren y
    /// [`MoneyError::Overflow`] si la suma queda fuera de `i64`.
    pub fn checked_add(&self, other: &Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        let amount = self
            .amount
            .checked_add(other.amount)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::new(amount, self.currency.clone()))
    }

    /// Importa un decimal ASCII exacto a unidades menores con escala explícita.
    ///
    /// Sintaxis: signo `+` o `-` opcional, uno o más dígitos y una parte decimal
    /// opcional con punto y uno o más dígitos. No admite espacios, exponentes,
    /// separadores de miles ni coma decimal. Admite ceros iniciales.
    ///
    /// La escala admitida es `0..=18`, cuyo factor `10^escala` cabe en `i64`.
    /// Los decimales ausentes se completan con ceros y los sobrantes solo se
    /// aceptan si son cero. No usa coma flotante ni aplica redondeo. La escala
    /// no se deduce del código de moneda ni valida un esquema SQL concreto.
    ///
    /// # Errors
    /// Devuelve [`MoneyError::InvalidScale`], [`MoneyError::InvalidDecimal`],
    /// [`MoneyError::NonRepresentableFraction`] o [`MoneyError::Overflow`].
    pub fn from_decimal_str(
        input: &str,
        currency: Currency,
        scale: u32,
    ) -> Result<Self, MoneyError> {
        if scale > 18 {
            return Err(MoneyError::InvalidScale);
        }
        let (negative, unsigned) = if let Some(value) = input.strip_prefix('-') {
            (true, value)
        } else if let Some(value) = input.strip_prefix('+') {
            (false, value)
        } else {
            (false, input)
        };
        let (integer, fraction) = match unsigned.split_once('.') {
            Some((integer, fraction)) => {
                if fraction.is_empty() {
                    return Err(MoneyError::InvalidDecimal);
                }
                (integer, fraction)
            }
            None => (unsigned, ""),
        };
        if integer.is_empty()
            || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(MoneyError::InvalidDecimal);
        }
        let scale = usize::try_from(scale).map_err(|_| MoneyError::InvalidScale)?;
        if fraction.bytes().skip(scale).any(|byte| byte != b'0') {
            return Err(MoneyError::NonRepresentableFraction);
        }
        let fraction_digits = fraction.len().min(scale);
        let digits = integer
            .bytes()
            .chain(fraction.bytes().take(scale))
            .chain(std::iter::repeat_n(b'0', scale - fraction_digits));
        let mut amount = 0_i64;
        for byte in digits {
            let digit = i64::from(byte - b'0');
            let shifted = amount.checked_mul(10).ok_or(MoneyError::Overflow)?;
            // Acumular con el signo final permite representar i64::MIN sin negarlo.
            amount = if negative {
                shifted.checked_sub(digit)
            } else {
                shifted.checked_add(digit)
            }
            .ok_or(MoneyError::Overflow)?;
        }
        Ok(Self::new(amount, currency))
    }
}

/// Fallos monetarios tipados que no conservan los datos de entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoneyError {
    /// El código no contiene exactamente tres letras ASCII.
    InvalidCurrency,
    /// Los importes pertenecen a monedas diferentes.
    CurrencyMismatch,
    /// El resultado no cabe en un entero firmado de 64 bits.
    Overflow,
    /// La escala solicitada queda fuera de `0..=18`.
    InvalidScale,
    /// La entrada no cumple la sintaxis decimal ASCII documentada.
    InvalidDecimal,
    /// La fracción no puede expresarse con la escala solicitada sin redondeo.
    NonRepresentableFraction,
}

impl fmt::Display for MoneyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCurrency => "el código debe contener tres letras ASCII",
            Self::CurrencyMismatch => "las monedas no coinciden",
            Self::Overflow => "el importe excede el rango de i64",
            Self::InvalidScale => "la escala debe estar entre 0 y 18",
            Self::InvalidDecimal => "sintaxis decimal no válida",
            Self::NonRepresentableFraction => "fracción no representable en la escala solicitada",
        })
    }
}

impl std::error::Error for MoneyError {}

#[cfg(test)]
mod audit_tests;
#[cfg(test)]
mod organization_branch_tests;
#[cfg(test)]
mod tests;
