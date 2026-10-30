use super::SaleError;

/// Estado operativo de una venta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaleStatus {
    /// Venta en construcción.
    Open,
    /// Venta cobrada; estado final.
    Completed,
    /// Venta anulada; estado final.
    Cancelled,
}

impl SaleStatus {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Completed => "COMPLETED",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// Convierte un valor persistido sin aceptar estados desconocidos.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, SaleError> {
        match value {
            "OPEN" => Ok(Self::Open),
            "COMPLETED" => Ok(Self::Completed),
            "CANCELLED" => Ok(Self::Cancelled),
            _ => Err(SaleError::InvalidPersistedValue),
        }
    }
}

/// Estado de una línea de venta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaleItemStatus {
    /// La línea cuenta para el total.
    Active,
    /// La línea se conserva como historial y no cuenta.
    Removed,
}

impl SaleItemStatus {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Removed => "REMOVED",
        }
    }

    /// Convierte un valor persistido sin aceptar estados desconocidos.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, SaleError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "REMOVED" => Ok(Self::Removed),
            _ => Err(SaleError::InvalidPersistedValue),
        }
    }
}
