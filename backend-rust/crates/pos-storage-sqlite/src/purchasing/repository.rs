//! Adaptador SQLite de compras: repositorio y unidad de trabajo.
//!
//! Implementa los puertos de `pos-application` sobre la **misma** transacción
//! física que la auditoría y el movimiento de stock. Ninguna consulta abre un
//! pool adicional ni confirma por separado.

use std::future::Future;

use pos_application::{
    CasOutcome,
    purchasing::{PurchasingApplication, PurchasingTransaction, PurchasingUnitOfWork},
};
use pos_core::{
    Currency,
    audit::AuditOccurredAt,
    catalog::CatalogCode,
    identity::UtcSeconds,
    inventory::{InventoryItem, Quantity},
    organization_branch::{ApplicationError, EntityId, Status},
    purchasing::{
        PurchaseOrder, PurchaseOrderLine, PurchaseOrderStatus, PurchaseReceipt, PurchaseReceiptLine,
    },
    supplier::Supplier,
};
use sqlx::Row;

use super::{
    mapper::{
        InventoryItemRow, PurchaseOrderLineRow, PurchaseOrderRow, PurchaseReceiptLineRow,
        PurchaseReceiptRow, SupplierRow,
    },
    queries,
};
use crate::{SqliteTransaction, SqliteUnitOfWork, prelude::transaction_error};

/// Error de persistencia de compras sin SQL ni valores de entrada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PurchasingStorageError {
    /// Un slug de proveedor o código de orden ya existe en la organización.
    DuplicateCode,
    /// Una FK de ownership o de artículo fue rechazada.
    ForeignKey,
    /// La fila incumple un CHECK o no puede rehidratarse.
    InvalidRow,
    /// SQLite rechazó la operación por una causa no clasificable.
    Database,
}

impl std::fmt::Display for PurchasingStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::DuplicateCode => "código duplicado en compras",
            Self::ForeignKey => "referencia de compras inválida",
            Self::InvalidRow => "fila de compras inválida",
            Self::Database => "operación SQLite de compras fallida",
        })
    }
}

impl std::error::Error for PurchasingStorageError {}

fn to_application(error: PurchasingStorageError) -> ApplicationError {
    match error {
        PurchasingStorageError::DuplicateCode => ApplicationError::PurchaseOrderCodeAlreadyExists,
        PurchasingStorageError::ForeignKey => ApplicationError::OwnershipMismatch,
        PurchasingStorageError::InvalidRow | PurchasingStorageError::Database => {
            ApplicationError::Storage
        }
    }
}

fn map_write_error(error: &sqlx::Error) -> PurchasingStorageError {
    let Some(database) = error.as_database_error() else {
        return PurchasingStorageError::Database;
    };
    if database.is_unique_violation() {
        return PurchasingStorageError::DuplicateCode;
    }
    if database.is_foreign_key_violation() {
        return PurchasingStorageError::ForeignKey;
    }
    if database.is_check_violation() {
        return PurchasingStorageError::InvalidRow;
    }
    PurchasingStorageError::Database
}

fn invalid_row() -> ApplicationError {
    ApplicationError::Storage
}

/// Alias de compatibilidad: el caso de uso vive en `pos-application`.
pub type SqlitePurchasingApplication = PurchasingApplication<SqliteUnitOfWork>;

impl PurchasingUnitOfWork for SqliteUnitOfWork {
    type Transaction = SqliteTransaction;

    async fn begin_transaction(&self) -> Result<SqliteTransaction, ApplicationError> {
        SqliteUnitOfWork::begin(self)
            .await
            .map_err(transaction_error)
    }

    async fn commit_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.commit().await.map_err(transaction_error)
    }

    async fn rollback_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.rollback().await.map_err(transaction_error)
    }
}

impl SqliteTransaction {
    async fn currency_of(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Option<Currency>, ApplicationError> {
        let value: Option<String> = sqlx::query_scalar(queries::SELECT_ORGANIZATION_CURRENCY)
            .bind(organization_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        value
            .map(|code| Currency::new(&code).map_err(|_| invalid_row()))
            .transpose()
    }

    async fn supplier_row(
        &mut self,
        organization_id: &EntityId,
        supplier_id: &EntityId,
    ) -> Result<Option<Supplier>, ApplicationError> {
        let row = sqlx::query_as::<_, SupplierRow>(queries::SELECT_SUPPLIER)
            .bind(organization_id.as_str())
            .bind(supplier_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        row.map(SupplierRow::into_domain)
            .transpose()
            .map_err(|_| invalid_row())
    }

    async fn order_row(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> Result<Option<PurchaseOrder>, ApplicationError> {
        let row = sqlx::query_as::<_, PurchaseOrderRow>(queries::SELECT_PURCHASE_ORDER)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(purchase_order_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        row.map(PurchaseOrderRow::into_domain)
            .transpose()
            .map_err(|_| invalid_row())
    }

    async fn lines_of(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> Result<Vec<PurchaseOrderLine>, ApplicationError> {
        let rows = sqlx::query_as::<_, PurchaseOrderLineRow>(queries::SELECT_ORDER_LINES)
            .bind(organization_id.as_str())
            .bind(purchase_order_id.as_str())
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter()
            .map(PurchaseOrderLineRow::into_domain)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| invalid_row())
    }

    async fn receipt_lines_of(
        &mut self,
        organization_id: &EntityId,
        purchase_receipt_id: &EntityId,
    ) -> Result<Vec<PurchaseReceiptLine>, ApplicationError> {
        let rows = sqlx::query_as::<_, PurchaseReceiptLineRow>(queries::SELECT_RECEIPT_LINES)
            .bind(organization_id.as_str())
            .bind(purchase_receipt_id.as_str())
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter()
            .map(PurchaseReceiptLineRow::into_domain)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| invalid_row())
    }

    /// Suma exacta de cantidades recibidas por partida.
    async fn received_totals(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> Result<Vec<(EntityId, Quantity)>, ApplicationError> {
        let rows = sqlx::query(queries::SELECT_RECEIVED_LINES)
            .bind(organization_id.as_str())
            .bind(purchase_order_id.as_str())
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let mut totals: Vec<(EntityId, Quantity)> = Vec::new();
        for row in rows {
            let line_id: String = row.try_get("line_id").map_err(|_| invalid_row())?;
            let quantity: String = row.try_get("quantity").map_err(|_| invalid_row())?;
            let line_id = EntityId::parse(&line_id).map_err(|_| invalid_row())?;
            let quantity = Quantity::parse(&quantity).map_err(|_| invalid_row())?;
            match totals.iter_mut().find(|(id, _)| id == &line_id) {
                Some((_, total)) => {
                    *total = total.checked_add(&quantity).map_err(|_| invalid_row())?;
                }
                None => totals.push((line_id, quantity)),
            }
        }
        Ok(totals)
    }
}

impl PurchasingTransaction for SqliteTransaction {
    fn utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send {
        let occurred_at = occurred_at.clone();
        async move {
            let seconds: Option<i64> =
                sqlx::query_scalar("SELECT CAST(strftime('%s', ?) AS INTEGER)")
                    .bind(occurred_at.as_str())
                    .fetch_optional(&mut *self.transaction)
                    .await
                    .map_err(|_| ApplicationError::Storage)?;
            let seconds = seconds.ok_or_else(invalid_row)?;
            UtcSeconds::new(seconds).map_err(|_| invalid_row())
        }
    }

    fn organization_currency(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Currency>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        async move { self.currency_of(&organization_id).await }
    }

    fn get_supplier(
        &mut self,
        organization_id: &EntityId,
        supplier_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Supplier>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let supplier_id = supplier_id.clone();
        async move { self.supplier_row(&organization_id, &supplier_id).await }
    }

    fn supplier_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let code = code.clone();
        async move {
            let found: Option<i64> = sqlx::query_scalar(queries::SELECT_SUPPLIER_BY_CODE)
                .bind(organization_id.as_str())
                .bind(code.as_str())
                .fetch_optional(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Ok(found.is_some())
        }
    }

    fn list_suppliers(
        &mut self,
        organization_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Supplier>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        async move {
            let status = status.map(|value| value.as_str());
            let rows = sqlx::query_as::<_, SupplierRow>(queries::SELECT_SUPPLIERS)
                .bind(organization_id.as_str())
                .bind(status)
                .bind(status)
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            rows.into_iter()
                .map(SupplierRow::into_domain)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| invalid_row())
        }
    }

    fn insert_supplier(
        &mut self,
        supplier: &Supplier,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let supplier = supplier.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            let result = sqlx::query(queries::INSERT_SUPPLIER)
                .bind(supplier.id().as_str())
                .bind(supplier.organization_id().as_str())
                .bind(supplier.code().as_str())
                .bind(supplier.name().as_str())
                .bind(supplier.legal_name())
                .bind(supplier.tax_id())
                .bind(supplier.contact_name())
                .bind(supplier.email())
                .bind(supplier.phone())
                .bind(supplier.address())
                .bind(supplier.status().as_str())
                .bind(i64::try_from(supplier.revision()).map_err(|_| invalid_row())?)
                .bind(&occurred_at)
                .bind(&occurred_at)
                .execute(&mut *self.transaction)
                .await;
            result.map(|_| ()).map_err(|error| {
                let failure = map_write_error(&error);
                if matches!(failure, PurchasingStorageError::DuplicateCode) {
                    ApplicationError::SupplierCodeAlreadyExists
                } else {
                    to_application(failure)
                }
            })
        }
    }

    fn update_supplier_cas(
        &mut self,
        previous_revision: u64,
        supplier: &Supplier,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let supplier = supplier.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            let result = sqlx::query(queries::UPDATE_SUPPLIER_CAS)
                .bind(supplier.name().as_str())
                .bind(supplier.legal_name())
                .bind(supplier.tax_id())
                .bind(supplier.contact_name())
                .bind(supplier.email())
                .bind(supplier.phone())
                .bind(supplier.address())
                .bind(supplier.status().as_str())
                .bind(i64::try_from(supplier.revision()).map_err(|_| invalid_row())?)
                .bind(&occurred_at)
                .bind(supplier.organization_id().as_str())
                .bind(supplier.id().as_str())
                .bind(i64::try_from(previous_revision).map_err(|_| invalid_row())?)
                .execute(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Ok(if result.rows_affected() == 1 {
                CasOutcome::Updated
            } else {
                CasOutcome::Conflict
            })
        }
    }

    fn get_inventory_item(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<InventoryItem>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            let row = sqlx::query_as::<_, InventoryItemRow>(queries::SELECT_INVENTORY_ITEM)
                .bind(organization_id.as_str())
                .bind(inventory_item_id.as_str())
                .fetch_optional(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            row.map(InventoryItemRow::into_domain)
                .transpose()
                .map_err(|_| invalid_row())
        }
    }

    fn get_purchase_order(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> impl Future<
        Output = Result<Option<(PurchaseOrder, Vec<PurchaseOrderLine>)>, ApplicationError>,
    > + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let purchase_order_id = purchase_order_id.clone();
        async move {
            let Some(order) = self
                .order_row(&organization_id, &branch_id, &purchase_order_id)
                .await?
            else {
                return Ok(None);
            };
            let lines = self.lines_of(&organization_id, order.id()).await?;
            Ok(Some((order, lines)))
        }
    }

    fn list_purchase_orders(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        status: Option<PurchaseOrderStatus>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<(PurchaseOrder, Vec<PurchaseOrderLine>)>, ApplicationError>>
    + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        async move {
            let status = status.map(|value| value.as_str());
            let rows = sqlx::query_as::<_, PurchaseOrderRow>(queries::SELECT_PURCHASE_ORDERS)
                .bind(organization_id.as_str())
                .bind(branch_id.as_str())
                .bind(status)
                .bind(status)
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            let mut orders = Vec::with_capacity(rows.len());
            for row in rows {
                let order = row.into_domain().map_err(|_| invalid_row())?;
                let lines = self.lines_of(&organization_id, order.id()).await?;
                orders.push((order, lines));
            }
            Ok(orders)
        }
    }

    fn purchase_order_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let code = code.clone();
        async move {
            let found: Option<i64> = sqlx::query_scalar(queries::SELECT_PURCHASE_ORDER_BY_CODE)
                .bind(organization_id.as_str())
                .bind(code.as_str())
                .fetch_optional(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Ok(found.is_some())
        }
    }

    fn insert_purchase_order(
        &mut self,
        order: &PurchaseOrder,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let order = order.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            let result = sqlx::query(queries::INSERT_PURCHASE_ORDER)
                .bind(order.id().as_str())
                .bind(order.organization_id().as_str())
                .bind(order.branch_id().as_str())
                .bind(order.supplier_id().as_str())
                .bind(order.code().as_str())
                .bind(order.status().as_str())
                .bind(order.currency().as_str())
                .bind(order.supplier_code_snapshot().as_str())
                .bind(order.supplier_name_snapshot().as_str())
                .bind(order.created_by_user_id().as_str())
                .bind(order.issued_by_user_id().map(EntityId::as_str))
                .bind(order.cancelled_by_user_id().map(EntityId::as_str))
                .bind(order.issued_at().map(UtcSeconds::get))
                .bind(order.cancelled_at().map(UtcSeconds::get))
                .bind(i64::try_from(order.revision()).map_err(|_| invalid_row())?)
                .bind(&occurred_at)
                .bind(&occurred_at)
                .execute(&mut *self.transaction)
                .await;
            result.map(|_| ()).map_err(|error| {
                let failure = map_write_error(&error);
                if matches!(failure, PurchasingStorageError::DuplicateCode) {
                    ApplicationError::PurchaseOrderCodeAlreadyExists
                } else {
                    to_application(failure)
                }
            })
        }
    }

    fn update_purchase_order_cas(
        &mut self,
        previous_revision: u64,
        order: &PurchaseOrder,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let order = order.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            let result = sqlx::query(queries::UPDATE_PURCHASE_ORDER_CAS)
                .bind(order.supplier_id().as_str())
                .bind(order.status().as_str())
                .bind(order.supplier_code_snapshot().as_str())
                .bind(order.supplier_name_snapshot().as_str())
                .bind(order.issued_by_user_id().map(EntityId::as_str))
                .bind(order.cancelled_by_user_id().map(EntityId::as_str))
                .bind(order.issued_at().map(UtcSeconds::get))
                .bind(order.cancelled_at().map(UtcSeconds::get))
                .bind(i64::try_from(order.revision()).map_err(|_| invalid_row())?)
                .bind(&occurred_at)
                .bind(order.organization_id().as_str())
                .bind(order.branch_id().as_str())
                .bind(order.id().as_str())
                .bind(i64::try_from(previous_revision).map_err(|_| invalid_row())?)
                .execute(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Ok(if result.rows_affected() == 1 {
                CasOutcome::Updated
            } else {
                CasOutcome::Conflict
            })
        }
    }

    fn insert_purchase_order_line(
        &mut self,
        line: &PurchaseOrderLine,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let line = line.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            sqlx::query(queries::INSERT_ORDER_LINE)
                .bind(line.id().as_str())
                .bind(line.organization_id().as_str())
                .bind(line.purchase_order_id().as_str())
                .bind(line.inventory_item_id().as_str())
                .bind(i64::from(line.line_number()))
                .bind(line.item_code_snapshot().as_str())
                .bind(line.item_name_snapshot().as_str())
                .bind(line.unit_snapshot().as_str())
                .bind(line.quantity().to_string())
                .bind(line.unit_cost().map(|cost| cost.to_string()))
                .bind(&occurred_at)
                .bind(&occurred_at)
                .execute(&mut *self.transaction)
                .await
                .map(|_| ())
                .map_err(|error| to_application(map_write_error(&error)))
        }
    }

    fn update_purchase_order_line(
        &mut self,
        line: &PurchaseOrderLine,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let line = line.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            sqlx::query(queries::UPDATE_ORDER_LINE)
                .bind(line.quantity().to_string())
                .bind(line.unit_cost().map(|cost| cost.to_string()))
                .bind(&occurred_at)
                .bind(line.organization_id().as_str())
                .bind(line.purchase_order_id().as_str())
                .bind(line.id().as_str())
                .execute(&mut *self.transaction)
                .await
                .map(|_| ())
                .map_err(|error| to_application(map_write_error(&error)))
        }
    }

    fn delete_purchase_order_line(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
        line_id: &EntityId,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let purchase_order_id = purchase_order_id.clone();
        let line_id = line_id.clone();
        async move {
            sqlx::query(queries::DELETE_ORDER_LINE)
                .bind(organization_id.as_str())
                .bind(purchase_order_id.as_str())
                .bind(line_id.as_str())
                .execute(&mut *self.transaction)
                .await
                .map(|_| ())
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn next_line_number(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> impl Future<Output = Result<u32, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let purchase_order_id = purchase_order_id.clone();
        async move {
            let next: Option<i64> = sqlx::query_scalar(queries::SELECT_NEXT_LINE_NUMBER)
                .bind(organization_id.as_str())
                .bind(purchase_order_id.as_str())
                .fetch_optional(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            u32::try_from(next.unwrap_or(1)).map_err(|_| invalid_row())
        }
    }

    fn received_quantities(
        &mut self,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<(EntityId, Quantity)>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let purchase_order_id = purchase_order_id.clone();
        async move {
            self.received_totals(&organization_id, &purchase_order_id)
                .await
        }
    }

    fn find_receipt_by_idempotency(
        &mut self,
        organization_id: &EntityId,
        idempotency_key: &str,
    ) -> impl Future<Output = Result<Option<PurchaseReceipt>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let idempotency_key = idempotency_key.to_owned();
        async move {
            let row = sqlx::query_as::<_, PurchaseReceiptRow>(queries::SELECT_RECEIPT_BY_KEY)
                .bind(organization_id.as_str())
                .bind(&idempotency_key)
                .fetch_optional(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            row.map(PurchaseReceiptRow::into_domain)
                .transpose()
                .map_err(|_| invalid_row())
        }
    }

    fn get_purchase_receipt(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        purchase_receipt_id: &EntityId,
    ) -> impl Future<Output = Result<Option<PurchaseReceipt>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let purchase_receipt_id = purchase_receipt_id.clone();
        async move {
            let row = sqlx::query_as::<_, PurchaseReceiptRow>(queries::SELECT_RECEIPT)
                .bind(organization_id.as_str())
                .bind(branch_id.as_str())
                .bind(purchase_receipt_id.as_str())
                .fetch_optional(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            row.map(PurchaseReceiptRow::into_domain)
                .transpose()
                .map_err(|_| invalid_row())
        }
    }

    fn list_purchase_receipts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        purchase_order_id: Option<&EntityId>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<PurchaseReceipt>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let purchase_order_id = purchase_order_id.cloned();
        async move {
            let order_id = purchase_order_id.as_ref().map(EntityId::as_str);
            let rows = sqlx::query_as::<_, PurchaseReceiptRow>(queries::SELECT_RECEIPTS)
                .bind(organization_id.as_str())
                .bind(branch_id.as_str())
                .bind(order_id)
                .bind(order_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            rows.into_iter()
                .map(PurchaseReceiptRow::into_domain)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| invalid_row())
        }
    }

    fn receipt_lines(
        &mut self,
        organization_id: &EntityId,
        purchase_receipt_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<PurchaseReceiptLine>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let purchase_receipt_id = purchase_receipt_id.clone();
        async move {
            self.receipt_lines_of(&organization_id, &purchase_receipt_id)
                .await
        }
    }

    fn insert_purchase_receipt(
        &mut self,
        receipt: &PurchaseReceipt,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let receipt = receipt.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            sqlx::query(queries::INSERT_RECEIPT)
                .bind(receipt.id().as_str())
                .bind(receipt.organization_id().as_str())
                .bind(receipt.branch_id().as_str())
                .bind(receipt.purchase_order_id().as_str())
                .bind(receipt.idempotency_key())
                .bind(receipt.actor_user_id().as_str())
                .bind(receipt.received_at_seconds())
                .bind(&occurred_at)
                .execute(&mut *self.transaction)
                .await
                .map(|_| ())
                .map_err(|error| {
                    let failure = map_write_error(&error);
                    if matches!(failure, PurchasingStorageError::DuplicateCode) {
                        ApplicationError::DuplicateReceipt
                    } else {
                        to_application(failure)
                    }
                })
        }
    }

    fn insert_purchase_receipt_line(
        &mut self,
        line: &PurchaseReceiptLine,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let line = line.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            sqlx::query(queries::INSERT_RECEIPT_LINE)
                .bind(line.id().as_str())
                .bind(line.organization_id().as_str())
                .bind(line.purchase_receipt_id().as_str())
                .bind(line.purchase_order_line_id().as_str())
                .bind(line.inventory_item_id().as_str())
                .bind(line.quantity().to_string())
                .bind(&occurred_at)
                .execute(&mut *self.transaction)
                .await
                .map(|_| ())
                .map_err(|error| to_application(map_write_error(&error)))
        }
    }

    async fn apply_purchase_receipt_stock(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
        quantity: Quantity,
        receipt_line_id: &EntityId,
        ledger_id: &EntityId,
        balance_id: &EntityId,
        actor_user_id: &EntityId,
        occurred_at_seconds: i64,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        let current: Option<String> = sqlx::query_scalar(queries::SELECT_BALANCE)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(inventory_item_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let previous = match current {
            Some(value) => Quantity::parse(&value).map_err(|_| invalid_row())?,
            None => Quantity::zero(),
        };
        let resulting = previous
            .checked_add(&quantity)
            .map_err(|_| ApplicationError::Storage)?;
        sqlx::query(queries::UPSERT_BALANCE)
            .bind(balance_id.as_str())
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(inventory_item_id.as_str())
            .bind(resulting.to_string())
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| to_application(map_write_error(&error)))?;
        sqlx::query(queries::INSERT_LEDGER)
            .bind(ledger_id.as_str())
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(inventory_item_id.as_str())
            .bind(quantity.to_string())
            .bind(resulting.to_string())
            .bind(actor_user_id.as_str())
            .bind(occurred_at_seconds)
            .bind(receipt_line_id.as_str())
            .execute(&mut *self.transaction)
            .await
            .map(|_| ())
            .map_err(|error| to_application(map_write_error(&error)))
    }

    fn inventory_balance(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Quantity>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            let value: Option<String> = sqlx::query_scalar(queries::SELECT_BALANCE)
                .bind(organization_id.as_str())
                .bind(branch_id.as_str())
                .bind(inventory_item_id.as_str())
                .fetch_optional(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
            value
                .map(|text| Quantity::parse(&text).map_err(|_| invalid_row()))
                .transpose()
        }
    }
}
