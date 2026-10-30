//! DTOs del adaptador IPC; no se serializan agregados ni credenciales.
use serde::{Deserialize, Serialize};
use specta::Type;

/// Página explícita, sin organización ni actor controlados por el consumidor.
#[derive(Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageRequest {
    /// Límite de 1 a 100, validado por Application.
    pub limit: u32,
    /// Desplazamiento desde cero.
    pub offset: u32,
}

/// Resumen de una sucursal visible para el principal.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BranchDto {
    /// UUID canónico.
    pub id: String,
    /// Código de sucursal.
    pub code: String,
    /// Nombre visible.
    pub name: String,
    /// Estado operativo.
    pub status: String,
}

/// Categoría de catálogo, con versión exacta representada como texto.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    /// UUID canónico.
    pub id: String,
    /// Código estable.
    pub code: String,
    /// Nombre visible.
    pub name: String,
    /// Versión decimal entera, nunca number de JavaScript.
    pub version: String,
}

/// Producto administrativo con precio histórico exacto.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProductDto {
    /// UUID canónico.
    pub id: String,
    /// SKU canónico.
    pub sku: String,
    /// Nombre visible.
    pub name: String,
    /// Importe de escala 4, sin coma flotante.
    pub unit_price: String,
    /// Categoría asociada, si existe.
    pub category_id: Option<String>,
    /// Versión decimal entera.
    pub version: String,
}

/// Error seguro; nunca contiene SQL ni material de sesión.
#[derive(Debug, Serialize, Type)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcError {
    /// Sesión inexistente, vencida, revocada o scope no válido.
    Unauthorized,
    /// Principal válido sin permiso suficiente.
    Forbidden,
    /// Página fuera del contrato.
    ValidationError,
    /// Fallo interno, sin detalles del adaptador.
    InternalError,
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IpcError {}

impl From<pos_core::organization_branch::ApplicationError> for IpcError {
    fn from(error: pos_core::organization_branch::ApplicationError) -> Self {
        use pos_core::organization_branch::ApplicationError;
        match error {
            ApplicationError::Forbidden => Self::Forbidden,
            ApplicationError::InvalidCatalogInput => Self::ValidationError,
            _ => Self::InternalError,
        }
    }
}
