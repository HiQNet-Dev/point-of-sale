//! Mappers de filas SQLite a dominio para compras.
//!
//! Ningún mapper acepta valores fuera de contrato: si la fila incumple el
//! dominio, la lectura falla en vez de rehidratar un estado imposible.

use pos_core::{
    Currency,
    catalog::{CatalogCode, UnitPrice},
    identity::UtcSeconds,
    inventory::{InventoryBaseUnit, InventoryItem, Quantity},
    organization_branch::{DisplayName, EntityId, Status},
    purchasing::{
        PurchaseOrder, PurchaseOrderLine, PurchaseOrderStatus, PurchaseReceipt, PurchaseReceiptLine,
    },
    supplier::{
        BoundedText, MAX_ADDRESS_UNITS, MAX_CONTACT_NAME_UNITS, MAX_LEGAL_NAME_UNITS,
        MAX_PHONE_UNITS, MAX_TAX_ID_UNITS, Supplier, SupplierEmail,
    },
};
use sqlx::FromRow;

macro_rules! identifier {
    ($value:expr) => {
        EntityId::parse(&$value).map_err(|_| crate::StorageError::Database)?
    };
}

/// Fila de proveedor.
#[derive(FromRow)]
pub(crate) struct SupplierRow {
    pub id: String,
    pub organization_id: String,
    pub code: String,
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub status: String,
    pub version: i64,
}

impl SupplierRow {
    pub(crate) fn into_domain(self) -> Result<Supplier, crate::StorageError> {
        let status = Status::parse(&self.status).map_err(|_| crate::StorageError::Database)?;
        Ok(Supplier::restore(
            identifier!(self.id),
            identifier!(self.organization_id),
            CatalogCode::new(&self.code).map_err(|_| crate::StorageError::Database)?,
            DisplayName::new(&self.name).map_err(|_| crate::StorageError::Database)?,
            optional_legal(&self.legal_name)?,
            optional_tax_id(&self.tax_id)?,
            optional_contact(&self.contact_name)?,
            optional_email(&self.email)?,
            optional_phone(&self.phone)?,
            optional_address(&self.address)?,
            status,
            u64::try_from(self.version).map_err(|_| crate::StorageError::Database)?,
        ))
    }
}

fn optional_legal(
    value: &Option<String>,
) -> Result<Option<BoundedText<MAX_LEGAL_NAME_UNITS>>, crate::StorageError> {
    value
        .as_deref()
        .map(BoundedText::<MAX_LEGAL_NAME_UNITS>::restore)
        .transpose()
        .map_err(|_| crate::StorageError::Database)
}

fn optional_tax_id(
    value: &Option<String>,
) -> Result<Option<BoundedText<MAX_TAX_ID_UNITS>>, crate::StorageError> {
    value
        .as_deref()
        .map(BoundedText::<MAX_TAX_ID_UNITS>::restore)
        .transpose()
        .map_err(|_| crate::StorageError::Database)
}

fn optional_contact(
    value: &Option<String>,
) -> Result<Option<BoundedText<MAX_CONTACT_NAME_UNITS>>, crate::StorageError> {
    value
        .as_deref()
        .map(BoundedText::<MAX_CONTACT_NAME_UNITS>::restore)
        .transpose()
        .map_err(|_| crate::StorageError::Database)
}

fn optional_phone(
    value: &Option<String>,
) -> Result<Option<BoundedText<MAX_PHONE_UNITS>>, crate::StorageError> {
    value
        .as_deref()
        .map(BoundedText::<MAX_PHONE_UNITS>::restore)
        .transpose()
        .map_err(|_| crate::StorageError::Database)
}

fn optional_address(
    value: &Option<String>,
) -> Result<Option<BoundedText<MAX_ADDRESS_UNITS>>, crate::StorageError> {
    value
        .as_deref()
        .map(BoundedText::<MAX_ADDRESS_UNITS>::restore)
        .transpose()
        .map_err(|_| crate::StorageError::Database)
}

fn optional_email(value: &Option<String>) -> Result<Option<SupplierEmail>, crate::StorageError> {
    value
        .as_deref()
        .map(SupplierEmail::restore)
        .transpose()
        .map_err(|_| crate::StorageError::Database)
}

/// Fila de orden de compra.
#[derive(FromRow)]
pub(crate) struct PurchaseOrderRow {
    pub id: String,
    pub organization_id: String,
    pub branch_id: String,
    pub supplier_id: String,
    pub code: String,
    pub status: String,
    pub currency: String,
    pub supplier_code_snapshot: String,
    pub supplier_name_snapshot: String,
    pub created_by_user_id: String,
    pub issued_by_user_id: Option<String>,
    pub cancelled_by_user_id: Option<String>,
    pub issued_at: Option<i64>,
    pub cancelled_at: Option<i64>,
    pub version: i64,
}

impl PurchaseOrderRow {
    pub(crate) fn into_domain(self) -> Result<PurchaseOrder, crate::StorageError> {
        PurchaseOrder::restore(
            identifier!(self.id),
            identifier!(self.organization_id),
            identifier!(self.branch_id),
            identifier!(self.supplier_id),
            CatalogCode::new(&self.code).map_err(|_| crate::StorageError::Database)?,
            PurchaseOrderStatus::parse(&self.status).map_err(|_| crate::StorageError::Database)?,
            Currency::new(&self.currency).map_err(|_| crate::StorageError::Database)?,
            CatalogCode::new(&self.supplier_code_snapshot)
                .map_err(|_| crate::StorageError::Database)?,
            DisplayName::new(&self.supplier_name_snapshot)
                .map_err(|_| crate::StorageError::Database)?,
            identifier!(self.created_by_user_id),
            optional_identifier(self.issued_by_user_id)?,
            optional_identifier(self.cancelled_by_user_id)?,
            self.issued_at.map(seconds).transpose()?,
            self.cancelled_at.map(seconds).transpose()?,
            u64::try_from(self.version).map_err(|_| crate::StorageError::Database)?,
        )
        .map_err(|_| crate::StorageError::Database)
    }
}

/// Fila de partida.
#[derive(FromRow)]
pub(crate) struct PurchaseOrderLineRow {
    pub id: String,
    pub organization_id: String,
    pub purchase_order_id: String,
    pub inventory_item_id: String,
    pub line_number: i64,
    pub item_code_snapshot: String,
    pub item_name_snapshot: String,
    pub unit_snapshot: String,
    pub quantity: String,
    pub unit_cost: Option<String>,
}

impl PurchaseOrderLineRow {
    pub(crate) fn into_domain(self) -> Result<PurchaseOrderLine, crate::StorageError> {
        PurchaseOrderLine::restore(
            identifier!(self.id),
            identifier!(self.organization_id),
            identifier!(self.purchase_order_id),
            identifier!(self.inventory_item_id),
            u32::try_from(self.line_number).map_err(|_| crate::StorageError::Database)?,
            CatalogCode::new(&self.item_code_snapshot)
                .map_err(|_| crate::StorageError::Database)?,
            DisplayName::new(&self.item_name_snapshot)
                .map_err(|_| crate::StorageError::Database)?,
            InventoryBaseUnit::parse(&self.unit_snapshot)
                .map_err(|_| crate::StorageError::Database)?,
            Quantity::parse(&self.quantity).map_err(|_| crate::StorageError::Database)?,
            self.unit_cost
                .as_deref()
                .map(UnitPrice::parse)
                .transpose()
                .map_err(|_| crate::StorageError::Database)?,
        )
        .map_err(|_| crate::StorageError::Database)
    }
}

/// Fila de recepción.
#[derive(FromRow)]
pub(crate) struct PurchaseReceiptRow {
    pub id: String,
    pub organization_id: String,
    pub branch_id: String,
    pub purchase_order_id: String,
    pub idempotency_key: String,
    pub actor_user_id: String,
    pub received_at: i64,
}

impl PurchaseReceiptRow {
    pub(crate) fn into_domain(self) -> Result<PurchaseReceipt, crate::StorageError> {
        PurchaseReceipt::restore(
            identifier!(self.id),
            identifier!(self.organization_id),
            identifier!(self.branch_id),
            identifier!(self.purchase_order_id),
            self.idempotency_key,
            identifier!(self.actor_user_id),
            self.received_at,
        )
        .map_err(|_| crate::StorageError::Database)
    }
}

/// Fila de línea de recepción.
#[derive(FromRow)]
pub(crate) struct PurchaseReceiptLineRow {
    pub id: String,
    pub organization_id: String,
    pub purchase_receipt_id: String,
    pub purchase_order_line_id: String,
    pub inventory_item_id: String,
    pub quantity: String,
}

impl PurchaseReceiptLineRow {
    pub(crate) fn into_domain(self) -> Result<PurchaseReceiptLine, crate::StorageError> {
        PurchaseReceiptLine::restore(
            identifier!(self.id),
            identifier!(self.organization_id),
            identifier!(self.purchase_receipt_id),
            identifier!(self.purchase_order_line_id),
            identifier!(self.inventory_item_id),
            Quantity::parse(&self.quantity).map_err(|_| crate::StorageError::Database)?,
        )
        .map_err(|_| crate::StorageError::Database)
    }
}

/// Fila mínima de artículo de inventario.
#[derive(FromRow)]
pub(crate) struct InventoryItemRow {
    pub id: String,
    pub organization_id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub base_unit: String,
    pub fractional_unit_allowed: i64,
    pub unit_locked: i64,
    pub status: String,
    pub version: i64,
}

impl InventoryItemRow {
    pub(crate) fn into_domain(self) -> Result<InventoryItem, crate::StorageError> {
        use pos_core::catalog::Description;
        use pos_core::inventory::InventoryItem;
        let description = match self.description.as_deref() {
            Some(value) => Description::new(value).map_err(|_| crate::StorageError::Database)?,
            None => None,
        };
        Ok(InventoryItem::restore(
            identifier!(self.id),
            identifier!(self.organization_id),
            CatalogCode::new(&self.code).map_err(|_| crate::StorageError::Database)?,
            DisplayName::new(&self.name).map_err(|_| crate::StorageError::Database)?,
            description,
            InventoryBaseUnit::parse(&self.base_unit).map_err(|_| crate::StorageError::Database)?,
            self.fractional_unit_allowed != 0,
            self.unit_locked != 0,
            Status::parse(&self.status).map_err(|_| crate::StorageError::Database)?,
            u64::try_from(self.version).map_err(|_| crate::StorageError::Database)?,
        ))
    }
}

fn seconds(value: i64) -> Result<UtcSeconds, crate::StorageError> {
    UtcSeconds::new(value).map_err(|_| crate::StorageError::Database)
}

fn optional_identifier(value: Option<String>) -> Result<Option<EntityId>, crate::StorageError> {
    value
        .as_deref()
        .map(EntityId::parse)
        .transpose()
        .map_err(|_| crate::StorageError::Database)
}
