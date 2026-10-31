//! DTOs de aplicación para compras.
//!
//! La capa de transporte (IPC/HTTP) construirá sus propios DTOs a partir de
//! estos; aquí solo se declara lo que los casos de uso necesitan. Las cantidades
//! y los importes viajan como texto decimal exacto, nunca como `f64`.

use pos_core::{
    inventory::{InventoryBaseUnit, Quantity},
    organization_branch::{EntityId, Status},
    purchasing::{PurchaseOrderStatus, PurchaseReceiptLine},
    supplier::{
        BoundedText, MAX_ADDRESS_UNITS, MAX_CONTACT_NAME_UNITS, MAX_LEGAL_NAME_UNITS,
        MAX_PHONE_UNITS, MAX_TAX_ID_UNITS, NewSupplier, Supplier, SupplierEmail, SupplierPatch,
    },
};

/// Página de listado con límites explícitos.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PurchasingPage {
    /// Tamaño de página ya validado por el caso de uso.
    pub limit: i64,
    /// Desplazamiento desde cero.
    pub offset: i64,
}

impl PurchasingPage {
    /// Valida los límites del contrato (1..=100, offset no negativo).
    ///
    /// # Errors
    /// Devuelve `false` si la página no cumple el contrato.
    pub fn is_valid(self) -> bool {
        (1..=super::MAX_PURCHASING_PAGE_SIZE).contains(&self.limit) && self.offset >= 0
    }
}

/// Alta de proveedor.
#[derive(Clone, Debug, Default)]
pub struct NewSupplierRequest {
    /// Slug inmutable.
    pub code: String,
    /// Nombre comercial.
    pub name: String,
    /// Razón social.
    pub legal_name: Option<String>,
    /// Identificador fiscal.
    pub tax_id: Option<String>,
    /// Contacto principal.
    pub contact_name: Option<String>,
    /// Email de contacto.
    pub email: Option<String>,
    /// Teléfono.
    pub phone: Option<String>,
    /// Dirección.
    pub address: Option<String>,
}

impl NewSupplierRequest {
    /// Construye el agregado de dominio validando cada campo.
    ///
    /// # Errors
    /// Devuelve `SupplierError` si un campo incumple su contrato.
    pub fn into_new_supplier(
        self,
        id: EntityId,
        organization_id: EntityId,
    ) -> Result<NewSupplier, pos_core::supplier::SupplierError> {
        Ok(NewSupplier {
            id,
            organization_id,
            code: pos_core::catalog::CatalogCode::new(&self.code)
                .map_err(|_| pos_core::supplier::SupplierError::InvalidPersistedValue)?,
            name: pos_core::organization_branch::DisplayName::new(&self.name)
                .map_err(|_| pos_core::supplier::SupplierError::TextTooLong)?,
            legal_name: text::<MAX_LEGAL_NAME_UNITS>(self.legal_name)?,
            tax_id: text::<MAX_TAX_ID_UNITS>(self.tax_id)?,
            contact_name: text::<MAX_CONTACT_NAME_UNITS>(self.contact_name)?,
            email: SupplierEmail::new(self.email.as_deref().unwrap_or(""))?,
            phone: text::<MAX_PHONE_UNITS>(self.phone)?,
            address: text::<MAX_ADDRESS_UNITS>(self.address)?,
        })
    }
}

fn text<const MAX: usize>(
    value: Option<String>,
) -> Result<Option<BoundedText<MAX>>, pos_core::supplier::SupplierError> {
    BoundedText::new(value.as_deref().unwrap_or(""))
}

/// Campo opcional con distinción explícita entre ausente y limpiado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionalField<T> {
    /// El cliente no envió el campo: se conserva.
    Absent,
    /// El cliente envió `null`: se limpia.
    Clear,
    /// El cliente envió un valor.
    Set(T),
}

impl<T> OptionalField<T> {
    /// Construye desde el transporte: `None` ausente, `Some(None)` limpiar.
    pub fn from_transport(value: Option<Option<T>>) -> Self {
        match value {
            None => Self::Absent,
            Some(None) => Self::Clear,
            Some(Some(value)) => Self::Set(value),
        }
    }
}
/// Actualización de proveedor con CAS.
#[derive(Clone, Debug)]
pub struct UpdateSupplierRequest {
    /// Revisión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo; `None` lo conserva.
    pub name: Option<String>,
    /// Razón social.
    pub legal_name: OptionalField<String>,
    /// Identificador fiscal.
    pub tax_id: OptionalField<String>,
    /// Contacto principal.
    pub contact_name: OptionalField<String>,
    /// Email de contacto.
    pub email: OptionalField<String>,
    /// Teléfono.
    pub phone: OptionalField<String>,
    /// Dirección.
    pub address: OptionalField<String>,
    /// Estado nuevo; `None` lo conserva.
    pub status: Option<Status>,
}

impl UpdateSupplierRequest {
    /// Construye el patch de dominio.
    ///
    /// # Errors
    /// Devuelve `SupplierError` si un campo incumple su contrato.
    pub fn into_patch(self) -> Result<SupplierPatch, pos_core::supplier::SupplierError> {
        Ok(SupplierPatch {
            expected_version: self.expected_version,
            name: match self.name {
                Some(value) => Some(
                    pos_core::organization_branch::DisplayName::new(&value)
                        .map_err(|_| pos_core::supplier::SupplierError::TextTooLong)?,
                ),
                None => None,
            },
            legal_name: optional_text::<MAX_LEGAL_NAME_UNITS>(self.legal_name)?,
            tax_id: optional_text::<MAX_TAX_ID_UNITS>(self.tax_id)?,
            contact_name: optional_text::<MAX_CONTACT_NAME_UNITS>(self.contact_name)?,
            email: match self.email {
                OptionalField::Absent => pos_core::catalog::PatchField::Absent,
                OptionalField::Clear => pos_core::catalog::PatchField::Clear,
                OptionalField::Set(value) => pos_core::catalog::PatchField::Set(
                    SupplierEmail::new(&value)?
                        .ok_or(pos_core::supplier::SupplierError::InvalidEmail)?,
                ),
            },
            phone: optional_text::<MAX_PHONE_UNITS>(self.phone)?,
            address: optional_text::<MAX_ADDRESS_UNITS>(self.address)?,
            status: self.status,
        })
    }
}

/// Convierte un campo opcional de transporte en el patch tipado del dominio.
fn optional_text<const MAX: usize>(
    field: OptionalField<String>,
) -> Result<pos_core::catalog::PatchField<BoundedText<MAX>>, pos_core::supplier::SupplierError> {
    Ok(match field {
        OptionalField::Absent => pos_core::catalog::PatchField::Absent,
        OptionalField::Clear => pos_core::catalog::PatchField::Clear,
        OptionalField::Set(value) => pos_core::catalog::PatchField::Set(
            BoundedText::<MAX>::new(&value)?
                .ok_or(pos_core::supplier::SupplierError::InvalidPersistedValue)?,
        ),
    })
}

// Límites de texto por campo, usados por `optional_text`.

/// Resumen de proveedor para listados.
#[derive(Clone, Debug)]
pub struct SupplierSummary {
    /// UUID.
    pub id: String,
    /// Slug.
    pub code: String,
    /// Nombre comercial.
    pub name: String,
    /// Email normalizado.
    pub email: Option<String>,
    /// Estado.
    pub status: String,
    /// Revisión.
    pub version: String,
}

impl From<&Supplier> for SupplierSummary {
    fn from(supplier: &Supplier) -> Self {
        Self {
            id: supplier.id().as_str().to_owned(),
            code: supplier.code().as_str().to_owned(),
            name: supplier.name().as_str().to_owned(),
            email: supplier.email().map(str::to_owned),
            status: supplier.status().as_str().to_owned(),
            version: supplier.revision().to_string(),
        }
    }
}

/// Nueva partida de una orden.
#[derive(Clone, Debug)]
pub struct NewPurchaseOrderLineRequest {
    /// Artículo de inventario de la organización.
    pub inventory_item_id: EntityId,
    /// Cantidad pedida, escala 6.
    pub quantity: String,
    /// Costo pactado: `None` = desconocido, `Some("0.0000")` = cero conocido.
    pub unit_cost: Option<String>,
}

/// Alta de orden de compra.
#[derive(Clone, Debug)]
pub struct NewPurchaseOrderRequest {
    /// Código de la orden.
    pub code: String,
    /// Proveedor de la organización.
    pub supplier_id: EntityId,
    /// Partidas iniciales; puede estar vacío.
    pub lines: Vec<NewPurchaseOrderLineRequest>,
}

/// Cambio de proveedor de un borrador.
#[derive(Clone, Debug)]
pub struct ChangeSupplierRequest {
    /// Revisión previamente leída.
    pub expected_version: u64,
    /// Proveedor nuevo.
    pub supplier_id: EntityId,
}

/// Edición de una partida.
#[derive(Clone, Debug)]
pub struct UpdatePurchaseOrderLineRequest {
    /// Revisión previamente leída de la orden.
    pub expected_version: u64,
    /// Cantidad nueva.
    pub quantity: String,
    /// Costo: `None` ausente, `Some(None)` limpia a desconocido.
    pub unit_cost: Option<Option<String>>,
}

/// Recepción solicitada.
#[derive(Clone, Debug)]
pub struct PostReceiptRequest {
    /// Revisión previamente leída de la orden.
    pub expected_version: u64,
    /// Clave de idempotencia.
    pub idempotency_key: String,
    /// Líneas a recibir.
    pub lines: Vec<ReceiptLineInput>,
}

/// Cantidad recibida de una partida.
#[derive(Clone, Debug)]
pub struct ReceiptLineInput {
    /// Partida de la orden.
    pub purchase_order_line_id: EntityId,
    /// Cantidad recibida, escala 6.
    pub quantity: String,
}

/// Línea de orden tal como se devuelve a la aplicación.
#[derive(Clone, Debug)]
pub struct PurchaseOrderLineView {
    /// UUID de la partida.
    pub id: String,
    /// Número de línea.
    pub line_number: u32,
    /// Artículo.
    pub inventory_item_id: String,
    /// Código congelado.
    pub item_code: String,
    /// Nombre congelado.
    pub item_name: String,
    /// Unidad congelada.
    pub unit: String,
    /// Cantidad pedida exacta.
    pub quantity: String,
    /// Costo pactado exacto; `None` es desconocido.
    pub unit_cost: Option<String>,
    /// Total de la partida; `None` si el costo es desconocido.
    pub line_total: Option<String>,
    /// Cantidad ya recibida exacta.
    pub received_quantity: String,
    /// Indica si la partida está completamente recibida.
    pub fully_received: bool,
}

/// Orden de compra con partidas y totales derivados.
#[derive(Clone, Debug)]
pub struct PurchaseOrderView {
    /// UUID.
    pub id: String,
    /// Sucursal.
    pub branch_id: String,
    /// Proveedor.
    pub supplier_id: String,
    /// Código de la orden.
    pub code: String,
    /// Estado.
    pub status: String,
    /// Moneda.
    pub currency: String,
    /// Snapshot del código de proveedor.
    pub supplier_code: String,
    /// Snapshot del nombre de proveedor.
    pub supplier_name: String,
    /// Revisión.
    pub version: String,
    /// Total derivado; `None` si alguna partida desconoce su costo.
    pub total: Option<String>,
    /// Partidas.
    pub lines: Vec<PurchaseOrderLineView>,
}

impl PurchaseOrderView {
    /// Estado persistido como texto.
    pub fn status_of(status: PurchaseOrderStatus) -> &'static str {
        status.as_str()
    }
}

/// Recepción devuelta a la aplicación.
#[derive(Clone, Debug)]
pub struct PurchaseReceiptView {
    /// UUID.
    pub id: String,
    /// Orden.
    pub purchase_order_id: String,
    /// Clave de idempotencia.
    pub idempotency_key: String,
    /// Instante Unix.
    pub received_at: String,
    /// Líneas.
    pub lines: Vec<PurchaseReceiptLineView>,
}

/// Línea de recepción devuelta.
#[derive(Clone, Debug)]
pub struct PurchaseReceiptLineView {
    /// Partida de la orden.
    pub purchase_order_line_id: String,
    /// Artículo.
    pub inventory_item_id: String,
    /// Cantidad recibida.
    pub quantity: String,
}

impl From<&PurchaseReceiptLine> for PurchaseReceiptLineView {
    fn from(line: &PurchaseReceiptLine) -> Self {
        Self {
            purchase_order_line_id: line.purchase_order_line_id().as_str().to_owned(),
            inventory_item_id: line.inventory_item_id().as_str().to_owned(),
            quantity: line.quantity().to_string(),
        }
    }
}

/// Cantidad pedida y recibida por partida, para el estado de recepción.
#[derive(Clone, Debug)]
pub struct ReceiptStatusView {
    /// Partida.
    pub purchase_order_line_id: String,
    /// Cantidad pedida exacta.
    pub ordered_quantity: String,
    /// Cantidad recibida exacta.
    pub received_quantity: String,
    /// Cantidad pendiente exacta.
    pub pending_quantity: String,
    /// Indica si la partida está completa.
    pub fully_received: bool,
}

/// Unidad base como texto persistible.
pub fn unit_code(unit: InventoryBaseUnit) -> &'static str {
    unit.as_str()
}

/// Cantidad de la partida a partir de texto exacto.
///
/// # Errors
/// Devuelve `false` si el texto no es una cantidad positiva canónica.
pub fn parse_positive_quantity(value: &str) -> Option<Quantity> {
    Quantity::parse(value)
        .ok()
        .filter(|quantity| !quantity.is_zero() && !quantity.is_negative())
}

/// Importe de costo pactado.
///
/// # Errors
/// Devuelve `None` si el texto no es un importe de escala 4 no negativo.
pub fn parse_optional_cost(value: Option<&str>) -> Option<Option<pos_core::catalog::UnitPrice>> {
    match value {
        None => Some(None),
        Some(text) => pos_core::catalog::UnitPrice::parse(text).ok().map(Some),
    }
}
