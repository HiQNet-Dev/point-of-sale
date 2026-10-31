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

/// Petición de creación de producto; nunca incluye organización ni actor.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProductRequest {
    /// SKU canónico de la organización.
    pub sku: String,
    /// Nombre visible.
    pub name: String,
    /// Precio exacto como texto decimal de escala 4.
    pub unit_price: String,
    /// Categoría existente de la misma organización.
    pub category_id: Option<String>,
    /// Código de barras opcional.
    pub barcode: Option<String>,
    /// Descripción opcional.
    pub description: Option<String>,
}

/// Producto recién creado, con su revisión inicial.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreatedProductDto {
    /// UUID canónico asignado por la aplicación.
    pub id: String,
    /// SKU canónico.
    pub sku: String,
    /// Nombre visible.
    pub name: String,
    /// Importe exacto de escala 4.
    pub unit_price: String,
    /// Categoría asociada, si existe.
    pub category_id: Option<String>,
    /// Versión inicial del agregado.
    pub version: String,
}

/// Estado de la instalación persistente al abrir la aplicación.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallationStatusDto {
    /// `FIRST_INSTALL` si este arranque creó el esquema; `REOPENED` si ya existía.
    pub state: String,
    /// Versión de esquema aplicada, acotada al migrador aprobado del build.
    pub schema_version: u32,
    /// Migraciones aplicadas con éxito.
    pub applied_migrations: u32,
}

/// Credenciales presentadas por la pantalla de login.
///
/// La contraseña solo existe durante la llamada; el renderer nunca recibe el
/// handle de sesión ni un token que pueda almacenar. El `Debug` redacta el
/// secreto para que un registro accidental no lo exponga.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginRequestDto {
    /// Email tal como lo escribe la persona; se normaliza en backend.
    pub email: String,
    /// Contraseña en claro solo para esta llamada.
    pub password: String,
}

impl std::fmt::Debug for LoginRequestDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoginRequestDto")
            .field("email", &self.email)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Sucursal elegida por la persona autenticada.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectBranchRequestDto {
    /// UUID de una sucursal de la organización del principal.
    pub branch_id: String,
}

/// Identidad pública de la sesión y permisos efectivos.
///
/// No incluye handle, digest, hash de credencial ni código de sesión.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentityDto {
    /// Usuario autenticado.
    pub user_id: String,
    /// Organización derivada del usuario.
    pub organization_id: String,
    /// Nombre visible.
    pub display_name: String,
    /// Email normalizado.
    pub email: String,
    /// Sucursal operativa, si ya se seleccionó.
    pub branch_id: Option<String>,
    /// Permisos efectivos resueltos en SQLite.
    pub permissions: Vec<String>,
}

/// Resultado de cerrar todas las sesiones.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LogoutAllDto {
    /// Sesiones revocadas, incluida la actual.
    pub revoked: u32,
}

/// Identidad de una entidad del catálogo.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogItemRequestDto {
    /// UUID canónico dentro de la organización del principal.
    pub id: String,
}

/// Alta de categoría por el administrador autenticado.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewCategoryRequestDto {
    /// Código slug de la categoría.
    pub code: String,
    /// Nombre visible.
    pub name: String,
}

/// Edición de producto con versión exacta.
///
/// Los campos ausentes se conservan; el precio viaja como texto decimal de
/// escala 4 y nunca como número de JavaScript.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateProductRequestDto {
    /// UUID del producto.
    pub product_id: String,
    /// Versión leída por el cliente, en texto decimal.
    pub expected_version: String,
    /// Nombre nuevo, si cambia.
    pub name: Option<String>,
    /// Precio nuevo exacto, si cambia.
    pub unit_price: Option<String>,
    /// Categoría nueva, si cambia.
    pub category_id: Option<String>,
    /// `ACTIVE` o `INACTIVE`, si cambia.
    pub status: Option<String>,
}

/// Estado del asistente de primera instalación.
///
/// No incluye tokens, digests ni rutas. `expiresAt` es el fin de la ventana de
/// la autorización nativa, en segundos Unix.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapStatusDto {
    /// `READY`, `AWAITING_AUTHORIZATION`, `AUTHORIZATION_EXPIRED`,
    /// `AUTHORIZATION_REVOKED`, `AUTHORIZATION_CONSUMED`, `INVALID_AUTHORIZATION`,
    /// `INSECURE_PERMISSIONS`, `UNSUPPORTED_PLATFORM` o `ALREADY_PROVISIONED`.
    pub state: String,
    /// Fin de la ventana de autorización, en segundos Unix, si el estado está listo.
    pub expires_at: Option<u32>,
}

/// Datos de la primera instalación.
///
/// La contraseña initial se verifica, hashea y persiste en Rust; el token de
/// autorización nunca forma parte de este contrato.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BootstrapInstallRequestDto {
    /// Nombre visible del negocio.
    pub business_name: String,
    /// Código slug opcional; si falta se deriva del nombre.
    pub business_code: Option<String>,
    /// Nombre visible del administrador.
    pub administrator_name: String,
    /// Correo del administrador.
    pub email: String,
    /// Contraseña inicial en claro solo para esta llamada.
    pub password: String,
    /// Moneda ISO de la organización; por defecto `MXN`.
    pub currency: Option<String>,
}

impl std::fmt::Debug for BootstrapInstallRequestDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BootstrapInstallRequestDto")
            .field("business_name", &self.business_name)
            .field("business_code", &self.business_code)
            .field("administrator_name", &self.administrator_name)
            .field("email", &self.email)
            .field("currency", &self.currency)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Error seguro; nunca contiene SQL ni material de sesión.
#[derive(Debug, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcError {
    /// Sesión inexistente, vencida, revocada o scope no válido.
    Unauthorized,
    /// Principal válido sin permiso suficiente.
    Forbidden,
    /// Entrada, página o cantidad fuera del contrato.
    ValidationError,
    /// CAS, código duplicado, idempotencia o estado incompatible.
    Conflict,
    /// La entidad referenciada no existe en el scope del principal.
    NotFound,
    /// La operación no existe en el modo actual de la aplicación.
    OperationNotAvailable,
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
            ApplicationError::Forbidden | ApplicationError::OwnershipMismatch => Self::Forbidden,
            ApplicationError::InvalidCredentials | ApplicationError::Unauthenticated => {
                Self::Unauthorized
            }
            ApplicationError::InvalidSessionInput
            | ApplicationError::InvalidCatalogInput
            | ApplicationError::InvalidPurchaseInput
            | ApplicationError::OverReceipt => Self::ValidationError,
            ApplicationError::BranchNotFound
            | ApplicationError::SupplierNotFound
            | ApplicationError::PurchaseOrderNotFound
            | ApplicationError::PurchaseOrderLineNotFound
            | ApplicationError::PurchaseReceiptNotFound
            | ApplicationError::InventoryItemNotFound => Self::NotFound,
            ApplicationError::ConcurrentModification
            | ApplicationError::SupplierCodeAlreadyExists
            | ApplicationError::PurchaseOrderCodeAlreadyExists
            | ApplicationError::SupplierInactive
            | ApplicationError::PurchaseOrderNotDraft
            | ApplicationError::PurchaseOrderAlreadyIssued
            | ApplicationError::PurchaseOrderAlreadyCancelled
            | ApplicationError::PurchaseOrderNotReceivable
            | ApplicationError::DuplicateReceipt => Self::Conflict,
            ApplicationError::CategoryNotFound | ApplicationError::ProductNotFound => {
                Self::NotFound
            }
            ApplicationError::CategoryCodeAlreadyExists
            | ApplicationError::ProductSkuAlreadyExists
            | ApplicationError::ProductBarcodeAlreadyExists => Self::Conflict,
            _ => Self::InternalError,
        }
    }
}
