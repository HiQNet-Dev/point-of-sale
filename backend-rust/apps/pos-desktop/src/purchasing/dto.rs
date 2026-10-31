//! Contratos IPC preparados; decimales y revisiones viajan como texto exacto.
use crate::dto::IpcError;
use pos_application::purchasing as app;
use pos_core::organization_branch::{EntityId, Status};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Patch explícito: no confunde ausencia, borrado y cero conocido.
#[derive(Debug, Default, Deserialize, Serialize, Type)]
#[serde(
    tag = "operation",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum TextPatch {
    /// Conservar el valor existente.
    #[default]
    Keep,
    /// Limpiar a desconocido.
    Clear,
    /// Establecer texto exacto.
    Set(String),
}
impl TextPatch {
    pub(super) fn nested(self) -> Option<Option<String>> {
        match self {
            Self::Keep => None,
            Self::Clear => Some(None),
            Self::Set(value) => Some(Some(value)),
        }
    }
    pub(super) fn field(self) -> app::OptionalField<String> {
        app::OptionalField::from_transport(self.nested())
    }
}

/// Estado operativo de proveedor.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupplierStatus {
    Active,
    Inactive,
}
impl SupplierStatus {
    pub(super) fn domain(self) -> Status {
        match self {
            Self::Active => Status::Active,
            Self::Inactive => Status::Inactive,
        }
    }
}

/// Alta de proveedor; el actor y organización nunca proceden del request.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplierInput {
    pub code: String,
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
}
impl From<SupplierInput> for app::NewSupplierRequest {
    fn from(value: SupplierInput) -> Self {
        Self {
            code: value.code,
            name: value.name,
            legal_name: value.legal_name,
            tax_id: value.tax_id,
            contact_name: value.contact_name,
            email: value.email,
            phone: value.phone,
            address: value.address,
        }
    }
}

/// Identificador scoped de proveedor.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplierKey {
    pub supplier_id: String,
}
/// Página de proveedores de la organización verificada.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplierList {
    pub limit: u32,
    pub offset: u32,
    pub status: Option<SupplierStatus>,
}
/// Actualización CAS; el slug no es mutable.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplierUpdate {
    pub supplier_id: String,
    pub expected_version: String,
    pub name: Option<String>,
    #[serde(default)]
    pub legal_name: TextPatch,
    #[serde(default)]
    pub tax_id: TextPatch,
    #[serde(default)]
    pub contact_name: TextPatch,
    #[serde(default)]
    pub email: TextPatch,
    #[serde(default)]
    pub phone: TextPatch,
    #[serde(default)]
    pub address: TextPatch,
    pub status: Option<SupplierStatus>,
}
impl SupplierUpdate {
    pub(super) fn application(self) -> Result<app::UpdateSupplierRequest, IpcError> {
        Ok(app::UpdateSupplierRequest {
            expected_version: version(&self.expected_version)?,
            name: self.name,
            legal_name: self.legal_name.field(),
            tax_id: self.tax_id.field(),
            contact_name: self.contact_name.field(),
            email: self.email.field(),
            phone: self.phone.field(),
            address: self.address.field(),
            status: self.status.map(SupplierStatus::domain),
        })
    }
}

/// Selección de orden y sucursal; el backend verifica el scope contra SQLite.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrderKey {
    pub branch_id: String,
    pub order_id: String,
}
/// Página y filtro de órdenes.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrderList {
    pub branch_id: String,
    pub limit: u32,
    pub offset: u32,
    pub status: Option<String>,
}
/// Partida con costo pactado opcional; nunca representa costo efectivo.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineInput {
    pub inventory_item_id: String,
    pub quantity: String,
    pub unit_cost: Option<String>,
}
impl LineInput {
    pub(super) fn application(self) -> Result<app::NewPurchaseOrderLineRequest, IpcError> {
        Ok(app::NewPurchaseOrderLineRequest {
            inventory_item_id: id(&self.inventory_item_id)?,
            quantity: self.quantity,
            unit_cost: self.unit_cost,
        })
    }
}
/// Creación de borrador.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrderInput {
    pub branch_id: String,
    pub code: String,
    pub supplier_id: String,
    pub lines: Vec<LineInput>,
}
/// Cambio de proveedor en borrador.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplierChange {
    pub branch_id: String,
    pub order_id: String,
    pub supplier_id: String,
    pub expected_version: String,
}
/// Alta de partida; la revisión se gestiona según el caso de uso existente.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineAdd {
    pub branch_id: String,
    pub order_id: String,
    pub line: LineInput,
}
/// Edición de cantidad y costo con revisión de la orden.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineUpdate {
    pub branch_id: String,
    pub order_id: String,
    pub line_id: String,
    pub expected_version: String,
    pub quantity: String,
    #[serde(default)]
    pub unit_cost: TextPatch,
}
/// Retirada de partida con CAS.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineRemove {
    pub branch_id: String,
    pub order_id: String,
    pub line_id: String,
    pub expected_version: String,
}
/// Transición irreversible con CAS.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrderTransition {
    pub branch_id: String,
    pub order_id: String,
    pub expected_version: String,
}
/// Cantidad recibida exacta por partida.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptLineInput {
    pub purchase_order_line_id: String,
    pub quantity: String,
}
/// Recepción idempotente; no admite costos efectivos inventados.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptInput {
    pub branch_id: String,
    pub order_id: String,
    pub expected_version: String,
    pub idempotency_key: String,
    pub lines: Vec<ReceiptLineInput>,
}
/// Consulta de recepción.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptKey {
    pub branch_id: String,
    pub receipt_id: String,
}
/// Página de recepciones con filtro opcional de orden.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptList {
    pub branch_id: String,
    pub order_id: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

pub(super) fn id(value: &str) -> Result<EntityId, IpcError> {
    EntityId::parse(value).map_err(|_| IpcError::ValidationError)
}
pub(super) fn version(value: &str) -> Result<u64, IpcError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| IpcError::ValidationError)?;
    if parsed.to_string() != value {
        return Err(IpcError::ValidationError);
    }
    Ok(parsed)
}
pub(super) fn page(limit: u32, offset: u32) -> app::PurchasingPage {
    app::PurchasingPage {
        limit: i64::from(limit),
        offset: i64::from(offset),
    }
}

/// Resumen de proveedor (sin credenciales o datos de sesión).
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SupplierDto {
    pub id: String,
    pub code: String,
    pub name: String,
    pub email: Option<String>,
    pub status: String,
    pub version: String,
}
impl From<app::SupplierSummary> for SupplierDto {
    fn from(v: app::SupplierSummary) -> Self {
        Self {
            id: v.id,
            code: v.code,
            name: v.name,
            email: v.email,
            status: v.status,
            version: v.version,
        }
    }
}
/// Partida con snapshots, cantidad escala 6 y costo escala 4.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OrderLineDto {
    pub id: String,
    pub line_number: u32,
    pub inventory_item_id: String,
    pub item_code: String,
    pub item_name: String,
    pub unit: String,
    pub quantity: String,
    pub unit_cost: Option<String>,
    pub line_total: Option<String>,
    pub received_quantity: String,
    pub fully_received: bool,
}
impl From<app::PurchaseOrderLineView> for OrderLineDto {
    fn from(v: app::PurchaseOrderLineView) -> Self {
        Self {
            id: v.id,
            line_number: v.line_number,
            inventory_item_id: v.inventory_item_id,
            item_code: v.item_code,
            item_name: v.item_name,
            unit: v.unit,
            quantity: v.quantity,
            unit_cost: v.unit_cost,
            line_total: v.line_total,
            received_quantity: v.received_quantity,
            fully_received: v.fully_received,
        }
    }
}
/// Orden con total desconocido si falta algún costo pactado.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OrderDto {
    pub id: String,
    pub branch_id: String,
    pub supplier_id: String,
    pub code: String,
    pub status: String,
    pub currency: String,
    pub supplier_code: String,
    pub supplier_name: String,
    pub version: String,
    pub total: Option<String>,
    pub lines: Vec<OrderLineDto>,
}
impl From<app::PurchaseOrderView> for OrderDto {
    fn from(v: app::PurchaseOrderView) -> Self {
        Self {
            id: v.id,
            branch_id: v.branch_id,
            supplier_id: v.supplier_id,
            code: v.code,
            status: v.status,
            currency: v.currency,
            supplier_code: v.supplier_code,
            supplier_name: v.supplier_name,
            version: v.version,
            total: v.total,
            lines: v.lines.into_iter().map(Into::into).collect(),
        }
    }
}
/// Cantidad recibida de una partida.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLineDto {
    pub purchase_order_line_id: String,
    pub inventory_item_id: String,
    pub quantity: String,
}
/// Recepción; receivedAt es Unix en texto exacto, no una fecha ISO implícita.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptDto {
    pub id: String,
    pub purchase_order_id: String,
    pub idempotency_key: String,
    pub received_at: String,
    pub lines: Vec<ReceiptLineDto>,
}
impl From<app::PurchaseReceiptView> for ReceiptDto {
    fn from(v: app::PurchaseReceiptView) -> Self {
        Self {
            id: v.id,
            purchase_order_id: v.purchase_order_id,
            idempotency_key: v.idempotency_key,
            received_at: v.received_at,
            lines: v
                .lines
                .into_iter()
                .map(|v| ReceiptLineDto {
                    purchase_order_line_id: v.purchase_order_line_id,
                    inventory_item_id: v.inventory_item_id,
                    quantity: v.quantity,
                })
                .collect(),
        }
    }
}
/// Acumulado exacto de recepción por partida.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptStatusDto {
    pub purchase_order_line_id: String,
    pub ordered_quantity: String,
    pub received_quantity: String,
    pub pending_quantity: String,
    pub fully_received: bool,
}
impl From<app::ReceiptStatusView> for ReceiptStatusDto {
    fn from(v: app::ReceiptStatusView) -> Self {
        Self {
            purchase_order_line_id: v.purchase_order_line_id,
            ordered_quantity: v.ordered_quantity,
            received_quantity: v.received_quantity,
            pending_quantity: v.pending_quantity,
            fully_received: v.fully_received,
        }
    }
}
