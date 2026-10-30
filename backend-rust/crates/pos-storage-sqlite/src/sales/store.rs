//! Casos de uso candidatos de ventas sobre SQLite real.
//!
//! No es código productivo: las tablas se aplican de modo explícito en pruebas
//! desde `proposals/local-sales/`. Cada mutación usa una sola UoW `BEGIN
//! IMMEDIATE`, conserva snapshots históricos y escribe auditoría antes del único
//! commit. Checkout no se expone aquí porque todavía no hay pagos e inventario
//! persistentes que puedan participar en su transacción atómica.

use std::{collections::BTreeMap, future::Future};

use pos_application::{
    audit::append_audit,
    authorization::require_sale_scope,
    sales_context::{SaleOperationalContext, require_open_operational_shift},
};
use pos_core::{
    audit::AuditOccurredAt,
    cash::{CashRegister, CashShift},
    catalog::{PriceDelta, UnitPrice},
    identity::{UtcSeconds, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, Branch, EntityId, IdentityGenerator, Permission, Status,
    },
    sales::{
        ModifierGroupAssignment, NewSale, NewSaleItem, Sale, SaleError, SaleItem, SaleItemStatus,
        SaleScope, SaleStatus, SelectedModifierOption, resolve_modifier_pricing,
        total_of_active_items,
    },
};
use serde_json::json;

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork, cash_store::OpenSalesForCashShift};

const SELECT_SALE: &str = "SELECT id, organization_id, branch_id, cash_register_id, cash_shift_id, \
    created_by_user_id, cancelled_by_user_id, completed_by_user_id, status, total_amount, version, \
    CAST(strftime('%s', cancelled_at) AS INTEGER) AS cancelled_at, \
    CAST(strftime('%s', completed_at) AS INTEGER) AS completed_at \
    FROM sale WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND cash_shift_id = ? AND id = ?";
const LIST_SALES: &str = "SELECT id, organization_id, branch_id, cash_register_id, cash_shift_id, \
    created_by_user_id, cancelled_by_user_id, completed_by_user_id, status, total_amount, version, \
    CAST(strftime('%s', cancelled_at) AS INTEGER) AS cancelled_at, \
    CAST(strftime('%s', completed_at) AS INTEGER) AS completed_at \
    FROM sale WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND cash_shift_id = ? \
    AND (? IS NULL OR status = ?) ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?";
const INSERT_SALE: &str = "INSERT INTO sale (id, organization_id, branch_id, cash_register_id, cash_shift_id, \
    created_by_user_id, cancelled_by_user_id, completed_by_user_id, status, total_amount, version, \
    created_at, updated_at, cancelled_at, completed_at) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, ?, ?, ?, NULL, NULL)";
const UPDATE_SALE_CAS: &str = "UPDATE sale SET cancelled_by_user_id = ?, completed_by_user_id = ?, status = ?, \
    total_amount = ?, version = ?, updated_at = ?, cancelled_at = ?, completed_at = ? \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND cash_shift_id = ? AND id = ? AND version = ?";
const SELECT_SALE_VERSION: &str = "SELECT version FROM sale WHERE organization_id = ? AND branch_id = ? \
    AND cash_register_id = ? AND cash_shift_id = ? AND id = ?";
const OPEN_SALE_EXISTS: &str =
    "SELECT EXISTS(SELECT 1 FROM sale WHERE cash_shift_id = ? AND status = 'OPEN')";

const SELECT_ITEM: &str = "SELECT id, organization_id, sale_id, product_id, product_sku_snapshot, product_name_snapshot, \
    base_unit_price, modifier_unit_delta, effective_unit_price, quantity, line_total, status, position, \
    removed_by_user_id, CAST(strftime('%s', removed_at) AS INTEGER) AS removed_at \
    FROM sale_item WHERE organization_id = ? AND sale_id = ? AND id = ?";
const LIST_ITEMS: &str = "SELECT id, organization_id, sale_id, product_id, product_sku_snapshot, product_name_snapshot, \
    base_unit_price, modifier_unit_delta, effective_unit_price, quantity, line_total, status, position, \
    removed_by_user_id, CAST(strftime('%s', removed_at) AS INTEGER) AS removed_at \
    FROM sale_item WHERE organization_id = ? AND sale_id = ? ORDER BY position, id";
const NEXT_ITEM_POSITION: &str = "SELECT COALESCE(MAX(position), -1) + 1 FROM sale_item WHERE organization_id = ? AND sale_id = ?";
const INSERT_ITEM: &str = "INSERT INTO sale_item (id, organization_id, sale_id, product_id, product_sku_snapshot, product_name_snapshot, \
    base_unit_price, modifier_unit_delta, effective_unit_price, quantity, line_total, status, position, created_at, updated_at, removed_at, removed_by_user_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL)";
const UPDATE_ITEM: &str = "UPDATE sale_item SET product_sku_snapshot = ?, product_name_snapshot = ?, base_unit_price = ?, \
    modifier_unit_delta = ?, effective_unit_price = ?, quantity = ?, line_total = ?, updated_at = ? \
    WHERE organization_id = ? AND sale_id = ? AND id = ? AND status = 'ACTIVE'";
const REMOVE_ITEM: &str = "UPDATE sale_item SET status = 'REMOVED', removed_by_user_id = ?, removed_at = ?, updated_at = ? \
    WHERE organization_id = ? AND sale_id = ? AND id = ? AND status = 'ACTIVE'";
const LIST_ITEM_OPTION_IDS: &str = "SELECT modifier_option_id FROM sale_item_modifier WHERE sale_item_id = ? ORDER BY modifier_option_id";
const DELETE_ITEM_MODIFIERS: &str = "DELETE FROM sale_item_modifier WHERE sale_item_id = ?";
const INSERT_ITEM_MODIFIER: &str = "INSERT INTO sale_item_modifier (id, organization_id, sale_item_id, modifier_group_id, modifier_option_id, \
    group_code_snapshot, group_name_snapshot, option_code_snapshot, option_name_snapshot, price_delta_snapshot, display_order, created_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
const SELECT_MODIFIER_CONFIGURATION: &str = "SELECT pmg.modifier_group_id, \
    mg.code AS group_code, mg.name AS group_name, mg.status AS group_status, mg.min_selections, mg.max_selections, \
    mo.id AS option_id, mo.code AS option_code, mo.name AS option_name, mo.status AS option_status, \
    mo.price_delta, mo.display_order AS option_display_order \
    FROM product_modifier_group pmg \
    JOIN modifier_group mg ON mg.id = pmg.modifier_group_id AND mg.organization_id = pmg.organization_id \
    LEFT JOIN modifier_option mo ON mo.modifier_group_id = mg.id AND mo.organization_id = mg.organization_id \
    WHERE pmg.organization_id = ? AND pmg.product_id = ? \
    ORDER BY pmg.display_order, pmg.modifier_group_id, mo.display_order, mo.id";

/// Tamaño máximo de página para el historial de ventas de un turno.
pub const MAX_SALE_PAGE_SIZE: u32 = 100;

/// Error de infraestructura de ventas candidato sin exponer SQL ni entradas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SalesStorageError {
    /// FK de scope, producto, turno o actor rechazada por SQLite.
    ForeignKey,
    /// CHECK, unicidad de posición o fila incoherente.
    InvalidRow,
    /// Error SQLite no clasificable.
    Database,
}

impl std::fmt::Display for SalesStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ForeignKey => "referencia de venta inválida",
            Self::InvalidRow => "fila de venta inválida",
            Self::Database => "operación SQLite de venta fallida",
        })
    }
}
impl std::error::Error for SalesStorageError {}

fn map_write_error(error: &sqlx::Error) -> SalesStorageError {
    let Some(database) = error.as_database_error() else {
        return SalesStorageError::Database;
    };
    if database.is_foreign_key_violation() {
        SalesStorageError::ForeignKey
    } else if database.is_check_violation() || database.is_unique_violation() {
        SalesStorageError::InvalidRow
    } else {
        SalesStorageError::Database
    }
}

fn revision_to_i64(value: u64) -> Result<i64, SalesStorageError> {
    i64::try_from(value).map_err(|_| SalesStorageError::InvalidRow)
}
fn revision_to_u64(value: i64) -> Result<u64, SalesStorageError> {
    u64::try_from(value).map_err(|_| SalesStorageError::InvalidRow)
}

#[derive(sqlx::FromRow)]
struct SaleRow {
    id: String,
    organization_id: String,
    branch_id: String,
    cash_register_id: String,
    cash_shift_id: String,
    created_by_user_id: String,
    cancelled_by_user_id: Option<String>,
    completed_by_user_id: Option<String>,
    status: String,
    total_amount: String,
    version: i64,
    cancelled_at: Option<i64>,
    completed_at: Option<i64>,
}

impl SaleRow {
    fn into_domain(self) -> Result<Sale, SalesStorageError> {
        Sale::restore(
            EntityId::parse(&self.id).map_err(|_| SalesStorageError::InvalidRow)?,
            SaleScope {
                organization_id: EntityId::parse(&self.organization_id)
                    .map_err(|_| SalesStorageError::InvalidRow)?,
                branch_id: EntityId::parse(&self.branch_id)
                    .map_err(|_| SalesStorageError::InvalidRow)?,
                cash_register_id: EntityId::parse(&self.cash_register_id)
                    .map_err(|_| SalesStorageError::InvalidRow)?,
                cash_shift_id: EntityId::parse(&self.cash_shift_id)
                    .map_err(|_| SalesStorageError::InvalidRow)?,
            },
            EntityId::parse(&self.created_by_user_id).map_err(|_| SalesStorageError::InvalidRow)?,
            self.cancelled_by_user_id
                .as_deref()
                .map(EntityId::parse)
                .transpose()
                .map_err(|_| SalesStorageError::InvalidRow)?,
            self.completed_by_user_id
                .as_deref()
                .map(EntityId::parse)
                .transpose()
                .map_err(|_| SalesStorageError::InvalidRow)?,
            SaleStatus::parse(&self.status).map_err(|_| SalesStorageError::InvalidRow)?,
            UnitPrice::parse(&self.total_amount).map_err(|_| SalesStorageError::InvalidRow)?,
            self.cancelled_at
                .map(UtcSeconds::new)
                .transpose()
                .map_err(|_| SalesStorageError::InvalidRow)?,
            self.completed_at
                .map(UtcSeconds::new)
                .transpose()
                .map_err(|_| SalesStorageError::InvalidRow)?,
            revision_to_u64(self.version)?,
        )
        .map_err(|_| SalesStorageError::InvalidRow)
    }
}

#[derive(sqlx::FromRow)]
struct SaleItemRow {
    id: String,
    organization_id: String,
    sale_id: String,
    product_id: String,
    product_sku_snapshot: String,
    product_name_snapshot: String,
    base_unit_price: String,
    modifier_unit_delta: String,
    effective_unit_price: String,
    quantity: i64,
    line_total: String,
    status: String,
    position: i64,
    removed_by_user_id: Option<String>,
    removed_at: Option<i64>,
}

impl SaleItemRow {
    fn into_domain(self) -> Result<SaleItem, SalesStorageError> {
        SaleItem::restore(
            EntityId::parse(&self.id).map_err(|_| SalesStorageError::InvalidRow)?,
            EntityId::parse(&self.organization_id).map_err(|_| SalesStorageError::InvalidRow)?,
            EntityId::parse(&self.sale_id).map_err(|_| SalesStorageError::InvalidRow)?,
            EntityId::parse(&self.product_id).map_err(|_| SalesStorageError::InvalidRow)?,
            pos_core::catalog::Sku::restore(&self.product_sku_snapshot)
                .map_err(|_| SalesStorageError::InvalidRow)?,
            pos_core::organization_branch::DisplayName::new(&self.product_name_snapshot)
                .map_err(|_| SalesStorageError::InvalidRow)?,
            UnitPrice::parse(&self.base_unit_price).map_err(|_| SalesStorageError::InvalidRow)?,
            PriceDelta::parse(&self.modifier_unit_delta)
                .map_err(|_| SalesStorageError::InvalidRow)?,
            UnitPrice::parse(&self.effective_unit_price)
                .map_err(|_| SalesStorageError::InvalidRow)?,
            u32::try_from(self.quantity).map_err(|_| SalesStorageError::InvalidRow)?,
            UnitPrice::parse(&self.line_total).map_err(|_| SalesStorageError::InvalidRow)?,
            SaleItemStatus::parse(&self.status).map_err(|_| SalesStorageError::InvalidRow)?,
            u32::try_from(self.position).map_err(|_| SalesStorageError::InvalidRow)?,
            self.removed_by_user_id
                .as_deref()
                .map(EntityId::parse)
                .transpose()
                .map_err(|_| SalesStorageError::InvalidRow)?,
            self.removed_at
                .map(UtcSeconds::new)
                .transpose()
                .map_err(|_| SalesStorageError::InvalidRow)?,
        )
        .map_err(|_| SalesStorageError::InvalidRow)
    }
}

#[derive(sqlx::FromRow)]
struct ModifierConfigurationRow {
    modifier_group_id: String,

    group_code: String,
    group_name: String,
    group_status: String,
    min_selections: i64,
    max_selections: i64,
    option_id: Option<String>,
    option_code: Option<String>,
    option_name: Option<String>,
    option_status: Option<String>,
    price_delta: Option<String>,
    option_display_order: Option<i64>,
}

struct ModifierSnapshot {
    group_id: EntityId,
    option_id: EntityId,
    group_code: String,
    group_name: String,
    option_code: String,
    option_name: String,
    price_delta: PriceDelta,
    active: bool,
    display_order: u32,
}

#[allow(missing_docs)]
impl SqliteTransaction {
    pub(crate) async fn get_sale(
        &mut self,
        scope: &SaleScope,
        sale_id: &EntityId,
    ) -> Result<Option<Sale>, SalesStorageError> {
        let row = sqlx::query_as::<_, SaleRow>(SELECT_SALE)
            .bind(scope.organization_id.as_str())
            .bind(scope.branch_id.as_str())
            .bind(scope.cash_register_id.as_str())
            .bind(scope.cash_shift_id.as_str())
            .bind(sale_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?;
        row.map(SaleRow::into_domain).transpose()
    }

    pub(crate) async fn list_sales(
        &mut self,
        scope: &SaleScope,
        status: Option<SaleStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Sale>, SalesStorageError> {
        let status = status.map(SaleStatus::as_str);
        sqlx::query_as::<_, SaleRow>(LIST_SALES)
            .bind(scope.organization_id.as_str())
            .bind(scope.branch_id.as_str())
            .bind(scope.cash_register_id.as_str())
            .bind(scope.cash_shift_id.as_str())
            .bind(status)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?
            .into_iter()
            .map(SaleRow::into_domain)
            .collect()
    }

    pub(crate) async fn insert_sale(
        &mut self,
        sale: &Sale,
        occurred_at: &str,
    ) -> Result<(), SalesStorageError> {
        sqlx::query(INSERT_SALE)
            .bind(sale.id().as_str())
            .bind(sale.scope().organization_id.as_str())
            .bind(sale.scope().branch_id.as_str())
            .bind(sale.scope().cash_register_id.as_str())
            .bind(sale.scope().cash_shift_id.as_str())
            .bind(sale.created_by_user_id().as_str())
            .bind(sale.status().as_str())
            .bind(sale.total_amount().to_string())
            .bind(revision_to_i64(sale.revision())?)
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?;
        Ok(())
    }

    pub(crate) async fn update_sale_cas(
        &mut self,
        previous: u64,
        sale: &Sale,
        occurred_at: &str,
    ) -> Result<CasOutcome, SalesStorageError> {
        let result = sqlx::query(UPDATE_SALE_CAS)
            .bind(sale.cancelled_by_user_id().map(EntityId::as_str))
            .bind(sale.completed_by_user_id().map(EntityId::as_str))
            .bind(sale.status().as_str())
            .bind(sale.total_amount().to_string())
            .bind(revision_to_i64(sale.revision())?)
            .bind(occurred_at)
            .bind(sale.cancelled_at().map(|_| occurred_at))
            .bind(sale.completed_at().map(|_| occurred_at))
            .bind(sale.scope().organization_id.as_str())
            .bind(sale.scope().branch_id.as_str())
            .bind(sale.scope().cash_register_id.as_str())
            .bind(sale.scope().cash_shift_id.as_str())
            .bind(sale.id().as_str())
            .bind(revision_to_i64(previous)?)
            .execute(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let exists: Option<i64> = sqlx::query_scalar(SELECT_SALE_VERSION)
            .bind(sale.scope().organization_id.as_str())
            .bind(sale.scope().branch_id.as_str())
            .bind(sale.scope().cash_register_id.as_str())
            .bind(sale.scope().cash_shift_id.as_str())
            .bind(sale.id().as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?;
        Ok(if exists.is_some() {
            CasOutcome::Conflict
        } else {
            CasOutcome::Missing
        })
    }

    pub(crate) async fn get_sale_item(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
        item_id: &EntityId,
    ) -> Result<Option<SaleItem>, SalesStorageError> {
        let row = sqlx::query_as::<_, SaleItemRow>(SELECT_ITEM)
            .bind(organization_id.as_str())
            .bind(sale_id.as_str())
            .bind(item_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?;
        row.map(SaleItemRow::into_domain).transpose()
    }

    pub(crate) async fn list_sale_items(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
    ) -> Result<Vec<SaleItem>, SalesStorageError> {
        sqlx::query_as::<_, SaleItemRow>(LIST_ITEMS)
            .bind(organization_id.as_str())
            .bind(sale_id.as_str())
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?
            .into_iter()
            .map(SaleItemRow::into_domain)
            .collect()
    }

    async fn next_sale_item_position(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
    ) -> Result<u32, SalesStorageError> {
        let position: i64 = sqlx::query_scalar(NEXT_ITEM_POSITION)
            .bind(organization_id.as_str())
            .bind(sale_id.as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?;
        u32::try_from(position).map_err(|_| SalesStorageError::InvalidRow)
    }

    async fn insert_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> Result<(), SalesStorageError> {
        sqlx::query(INSERT_ITEM)
            .bind(item.id().as_str())
            .bind(item.organization_id().as_str())
            .bind(item.sale_id().as_str())
            .bind(item.product_id().as_str())
            .bind(item.product_sku_snapshot().as_str())
            .bind(item.product_name_snapshot().as_str())
            .bind(item.base_unit_price().to_string())
            .bind(item.modifier_unit_delta().to_string())
            .bind(item.effective_unit_price().to_string())
            .bind(i64::from(item.quantity()))
            .bind(item.line_total().to_string())
            .bind(item.status().as_str())
            .bind(i64::from(item.position()))
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?;
        Ok(())
    }

    async fn update_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> Result<(), SalesStorageError> {
        let changed = sqlx::query(UPDATE_ITEM)
            .bind(item.product_sku_snapshot().as_str())
            .bind(item.product_name_snapshot().as_str())
            .bind(item.base_unit_price().to_string())
            .bind(item.modifier_unit_delta().to_string())
            .bind(item.effective_unit_price().to_string())
            .bind(i64::from(item.quantity()))
            .bind(item.line_total().to_string())
            .bind(occurred_at)
            .bind(item.organization_id().as_str())
            .bind(item.sale_id().as_str())
            .bind(item.id().as_str())
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?
            .rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(SalesStorageError::InvalidRow)
        }
    }

    async fn remove_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> Result<(), SalesStorageError> {
        let actor = item
            .removed_by_user_id()
            .ok_or(SalesStorageError::InvalidRow)?;
        let changed = sqlx::query(REMOVE_ITEM)
            .bind(actor.as_str())
            .bind(occurred_at)
            .bind(occurred_at)
            .bind(item.organization_id().as_str())
            .bind(item.sale_id().as_str())
            .bind(item.id().as_str())
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?
            .rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(SalesStorageError::InvalidRow)
        }
    }

    async fn item_option_ids(
        &mut self,
        item_id: &EntityId,
    ) -> Result<Vec<EntityId>, SalesStorageError> {
        let rows: Vec<String> = sqlx::query_scalar(LIST_ITEM_OPTION_IDS)
            .bind(item_id.as_str())
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| SalesStorageError::Database)?;
        rows.into_iter()
            .map(|value| EntityId::parse(&value).map_err(|_| SalesStorageError::InvalidRow))
            .collect()
    }

    async fn replace_item_modifiers<I: IdentityGenerator>(
        &mut self,
        identities: &mut I,
        item: &SaleItem,
        snapshots: &[ModifierSnapshot],
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(DELETE_ITEM_MODIFIERS)
            .bind(item.id().as_str())
            .execute(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        for snapshot in snapshots {
            sqlx::query(INSERT_ITEM_MODIFIER)
                .bind(identities.next_id()?.as_str())
                .bind(item.organization_id().as_str())
                .bind(item.id().as_str())
                .bind(snapshot.group_id.as_str())
                .bind(snapshot.option_id.as_str())
                .bind(&snapshot.group_code)
                .bind(&snapshot.group_name)
                .bind(&snapshot.option_code)
                .bind(&snapshot.option_name)
                .bind(snapshot.price_delta.to_string())
                .bind(i64::from(snapshot.display_order))
                .bind(occurred_at)
                .execute(&mut *self.transaction)
                .await
                .map_err(|_| ApplicationError::Storage)?;
        }
        Ok(())
    }

    async fn modifier_quote(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
        selected: &[EntityId],
        base: UnitPrice,
    ) -> Result<(PriceDelta, UnitPrice, Vec<ModifierSnapshot>), ApplicationError> {
        let rows = sqlx::query_as::<_, ModifierConfigurationRow>(SELECT_MODIFIER_CONFIGURATION)
            .bind(organization_id.as_str())
            .bind(product_id.as_str())
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let mut assignments = Vec::new();
        let mut snapshots = BTreeMap::new();
        for row in rows {
            let group_id =
                EntityId::parse(&row.modifier_group_id).map_err(|_| ApplicationError::Storage)?;
            if !assignments
                .iter()
                .any(|assignment: &ModifierGroupAssignment| assignment.group_id == group_id)
            {
                assignments.push(ModifierGroupAssignment {
                    group_id: group_id.clone(),
                    active: row.group_status == "ACTIVE",
                    min_selections: u32::try_from(row.min_selections)
                        .map_err(|_| ApplicationError::Storage)?,
                    max_selections: u32::try_from(row.max_selections)
                        .map_err(|_| ApplicationError::Storage)?,
                });
            }
            if let (
                Some(option_id),
                Some(option_code),
                Some(option_name),
                Some(option_status),
                Some(price_delta),
                Some(order),
            ) = (
                row.option_id,
                row.option_code,
                row.option_name,
                row.option_status,
                row.price_delta,
                row.option_display_order,
            ) {
                let option_id =
                    EntityId::parse(&option_id).map_err(|_| ApplicationError::Storage)?;
                snapshots.insert(
                    option_id.clone(),
                    ModifierSnapshot {
                        group_id: group_id.clone(),
                        option_id,
                        group_code: row.group_code,
                        group_name: row.group_name,
                        option_code,
                        option_name,
                        price_delta: PriceDelta::parse(&price_delta)
                            .map_err(|_| ApplicationError::Storage)?,
                        active: option_status == "ACTIVE",
                        display_order: u32::try_from(order)
                            .map_err(|_| ApplicationError::Storage)?,
                    },
                );
            }
        }
        let mut resolved = Vec::new();
        for option_id in selected {
            let snapshot = snapshots
                .get(option_id)
                .ok_or(ApplicationError::InvalidSaleInput)?;
            resolved.push(SelectedModifierOption {
                group_id: snapshot.group_id.clone(),
                option_id: option_id.clone(),
                active: snapshot.active,
                price_delta: snapshot.price_delta,
                display_order: snapshot.display_order,
            });
        }
        let pricing =
            resolve_modifier_pricing(base, &assignments, &resolved).map_err(sale_error)?;
        let ordered = pricing
            .ordered
            .into_iter()
            .map(|(_, option_id)| {
                snapshots
                    .remove(&option_id)
                    .ok_or(ApplicationError::Storage)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((pricing.delta, pricing.effective_unit_price, ordered))
    }
}

/// Adaptador concreto del puerto de cajas; consulta `sale` dentro de la misma UoW.
#[derive(Clone, Copy, Debug, Default)]
pub struct SqliteOpenSalesForCashShift;

impl OpenSalesForCashShift<SqliteTransaction> for SqliteOpenSalesForCashShift {
    fn has_open_sales(
        &self,
        transaction: &mut SqliteTransaction,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let cash_shift_id = cash_shift_id.clone();
        async move {
            let exists: i64 = sqlx::query_scalar(OPEN_SALE_EXISTS)
                .bind(cash_shift_id.as_str())
                .fetch_one(transaction.connection())
                .await
                .map_err(|_| ApplicationError::Storage)?;
            Ok(exists != 0)
        }
    }
}

/// Entrada para abrir una venta; el scope procede del principal y los IDs de caja/turno.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSaleRequest {
    /// Caja activa dentro de la sucursal del principal.
    pub cash_register_id: EntityId,
    /// Turno OPEN perteneciente a la caja indicada.
    pub cash_shift_id: EntityId,
}
/// Entrada para cotizar una partida; precios y snapshots son exclusivamente del servidor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddSaleItemRequest {
    /// Revisión leída de la venta antes de mutarla.
    pub expected_sale_version: u64,
    /// Producto de la misma organización que se cotiza en servidor.
    pub product_id: EntityId,
    /// Cantidad entera entre uno y el máximo canónico.
    pub quantity: u32,
    /// Opciones de modificador; nunca contiene precios ni snapshots del cliente.
    pub modifier_option_ids: Vec<EntityId>,
}
/// Entrada para recotizar una partida sin cambiar de producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateSaleItemRequest {
    /// Revisión leída de la venta antes de recotizar la partida.
    pub expected_sale_version: u64,
    /// Nueva cantidad entera de la partida activa.
    pub quantity: u32,
    /// Nueva selección completa de opciones de modificador.
    pub modifier_option_ids: Vec<EntityId>,
}
/// Página determinista de ventas de un turno.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SalePage {
    /// Máximo de filas, entre uno y [`MAX_SALE_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento en el orden histórico determinista.
    pub offset: u32,
}

/// Casos de uso candidatos de ventas. No ofrece checkout hasta que pagos e inventario sean atómicos.
#[derive(Clone)]
pub struct SqliteSalesApplication {
    unit_of_work: SqliteUnitOfWork,
}

impl SqliteSalesApplication {
    /// Ensambla ventas candidatas sobre una UoW SQLite configurada.
    pub fn new(unit_of_work: SqliteUnitOfWork) -> Self {
        Self { unit_of_work }
    }

    /// Abre una venta solo cuando sucursal, caja y turno están activos/abiertos.
    pub async fn create_sale<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewSaleRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Sale, ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleCreate,
            false,
            request.cash_register_id,
            request.cash_shift_id,
        )?;
        let sale_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let sale = Sale::open(NewSale { id: sale_id, scope: scope.clone(), created_by_user_id: principal.user_id().clone() });
            transaction.insert_sale(&sale, occurred_at.as_str()).await.map_err(storage_error)?;
            append_audit(&mut transaction, identities, principal, "sale.created", "sale", sale.id().clone(), json!({"cashRegisterId": scope.cash_register_id.as_str(), "cashShiftId": scope.cash_shift_id.as_str()}), occurred_at).await?;
            Ok(sale)
        }.await;
        finish(transaction, operation).await
    }

    /// Lee una venta y sus partidas dentro del scope del principal.
    pub async fn get_sale(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: EntityId,
        cash_shift_id: EntityId,
        sale_id: &EntityId,
    ) -> Result<(Sale, Vec<SaleItem>), ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleRead,
            false,
            cash_register_id,
            cash_shift_id,
        )?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let result = async {
            let sale = transaction
                .get_sale(&scope, sale_id)
                .await
                .map_err(storage_error)?
                .ok_or(ApplicationError::SaleNotFound)?;
            let items = transaction
                .list_sale_items(&scope.organization_id, sale_id)
                .await
                .map_err(storage_error)?;
            Ok((sale, items))
        }
        .await;
        finish(transaction, result).await
    }

    /// Lista ventas del turno scoped con filtro de estado y paginación acotada.
    pub async fn list_sales(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: EntityId,
        cash_shift_id: EntityId,
        status: Option<SaleStatus>,
        page: SalePage,
    ) -> Result<Vec<Sale>, ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleRead,
            false,
            cash_register_id,
            cash_shift_id,
        )?;
        if page.limit == 0 || page.limit > MAX_SALE_PAGE_SIZE {
            return Err(ApplicationError::InvalidSaleInput);
        }
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let result = transaction
            .list_sales(
                &scope,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await
            .map_err(storage_error);
        finish(transaction, result).await
    }

    /// Cotiza, persiste y audita una partida; actualiza el total de venta mediante CAS.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_sale_item<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: EntityId,
        cash_shift_id: EntityId,
        sale_id: &EntityId,
        request: AddSaleItemRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<(Sale, SaleItem), ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleUpdate,
            false,
            cash_register_id,
            cash_shift_id,
        )?;
        let item_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let mut sale = transaction.get_sale(&scope, sale_id).await.map_err(storage_error)?.ok_or(ApplicationError::SaleNotFound)?;
            sale.require_open().map_err(sale_error)?;
            if sale.revision() != request.expected_sale_version { return Err(ApplicationError::ConcurrentModification); }
            let product = transaction.get_product(&scope.organization_id, &request.product_id).await.map_err(|_| ApplicationError::Storage)?.ok_or(ApplicationError::ProductNotFound)?;
            if product.status() != Status::Active { return Err(ApplicationError::ProductInactive); }
            let (delta, effective, snapshots) = transaction.modifier_quote(&scope.organization_id, product.id(), &request.modifier_option_ids, product.unit_price()).await?;
            let position = transaction.next_sale_item_position(&scope.organization_id, sale_id).await.map_err(storage_error)?;
            let item = SaleItem::create(NewSaleItem { id: item_id, organization_id: scope.organization_id.clone(), sale_id: sale_id.clone(), product_id: product.id().clone(), product_sku_snapshot: product.sku().clone(), product_name_snapshot: product.name().clone(), base_unit_price: product.unit_price(), modifier_unit_delta: delta, effective_unit_price: effective, quantity: request.quantity, position }).map_err(sale_error)?;
            let mut all = transaction.list_sale_items(&scope.organization_id, sale_id).await.map_err(storage_error)?;
            all.push(item.clone());
            let total = total_of_active_items(&all).map_err(sale_error)?;
            let previous = sale.revision();
            sale.recalculate_total(total).map_err(sale_error)?;
            if transaction.update_sale_cas(previous, &sale, occurred_at.as_str()).await.map_err(storage_error)? != CasOutcome::Updated { return Err(ApplicationError::ConcurrentModification); }
            transaction.insert_sale_item(&item, occurred_at.as_str()).await.map_err(storage_error)?;
            transaction.replace_item_modifiers(identities, &item, &snapshots, occurred_at.as_str()).await?;
            append_audit(&mut transaction, identities, principal, "sale.item.added", "sale", sale.id().clone(), json!({"productId": product.id().as_str(), "quantity": item.quantity(), "lineTotal": item.line_total().to_string()}), occurred_at).await?;
            Ok((sale, item))
        }.await;
        finish(transaction, operation).await
    }

    /// Recotiza una partida activa y reemplaza sus snapshots en la misma UoW.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_sale_item<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: EntityId,
        cash_shift_id: EntityId,
        sale_id: &EntityId,
        item_id: &EntityId,
        request: UpdateSaleItemRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<(Sale, SaleItem), ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleUpdate,
            false,
            cash_register_id,
            cash_shift_id,
        )?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let mut sale = transaction.get_sale(&scope, sale_id).await.map_err(storage_error)?.ok_or(ApplicationError::SaleNotFound)?;
            sale.require_open().map_err(sale_error)?;
            if sale.revision() != request.expected_sale_version { return Err(ApplicationError::ConcurrentModification); }
            let mut item = transaction.get_sale_item(&scope.organization_id, sale_id, item_id).await.map_err(storage_error)?.ok_or(ApplicationError::SaleItemNotFound)?;
            item.require_active().map_err(sale_error)?;
            let product = transaction.get_product(&scope.organization_id, item.product_id()).await.map_err(|_| ApplicationError::Storage)?.ok_or(ApplicationError::ProductNotFound)?;
            if product.status() != Status::Active { return Err(ApplicationError::ProductInactive); }
            let (delta, effective, snapshots) = transaction.modifier_quote(&scope.organization_id, product.id(), &request.modifier_option_ids, product.unit_price()).await?;
            let previous_options = transaction.item_option_ids(item_id).await.map_err(storage_error)?;
            let mut requested_options = request.modifier_option_ids.clone();
            requested_options.sort();
            let same_options = previous_options == requested_options;
            if item.quantity() == request.quantity && same_options { return Ok((sale, item)); }
            item.update_quote(product.unit_price(), delta, effective, request.quantity).map_err(sale_error)?;
            let mut all = transaction.list_sale_items(&scope.organization_id, sale_id).await.map_err(storage_error)?;
            if let Some(existing) = all.iter_mut().find(|existing| existing.id() == item_id) { *existing = item.clone(); } else { return Err(ApplicationError::SaleItemNotFound); }
            let total = total_of_active_items(&all).map_err(sale_error)?;
            let previous = sale.revision(); sale.recalculate_total(total).map_err(sale_error)?;
            if transaction.update_sale_cas(previous, &sale, occurred_at.as_str()).await.map_err(storage_error)? != CasOutcome::Updated { return Err(ApplicationError::ConcurrentModification); }
            transaction.update_sale_item(&item, occurred_at.as_str()).await.map_err(storage_error)?;
            transaction.replace_item_modifiers(identities, &item, &snapshots, occurred_at.as_str()).await?;
            append_audit(&mut transaction, identities, principal, "sale.item.updated", "sale", sale.id().clone(), json!({"saleItemId": item_id.as_str(), "changedFields": ["quantity", "modifierOptionIds"]}), occurred_at).await?;
            Ok((sale, item))
        }.await;
        finish(transaction, operation).await
    }

    /// Retira lógicamente una partida activa y recalcula el total con CAS.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn remove_sale_item<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: EntityId,
        cash_shift_id: EntityId,
        sale_id: &EntityId,
        item_id: &EntityId,
        expected_sale_version: u64,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Sale, ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleUpdate,
            false,
            cash_register_id,
            cash_shift_id,
        )?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let mut sale = transaction
                .get_sale(&scope, sale_id)
                .await
                .map_err(storage_error)?
                .ok_or(ApplicationError::SaleNotFound)?;
            sale.require_open().map_err(sale_error)?;
            if sale.revision() != expected_sale_version {
                return Err(ApplicationError::ConcurrentModification);
            }
            let mut item = transaction
                .get_sale_item(&scope.organization_id, sale_id, item_id)
                .await
                .map_err(storage_error)?
                .ok_or(ApplicationError::SaleItemNotFound)?;
            item.remove(
                principal.user_id().clone(),
                transaction
                    .candidate_utc_seconds(occurred_at)
                    .await
                    .map_err(|_| ApplicationError::Storage)?,
            )
            .map_err(sale_error)?;
            let mut all = transaction
                .list_sale_items(&scope.organization_id, sale_id)
                .await
                .map_err(storage_error)?;
            if let Some(existing) = all.iter_mut().find(|existing| existing.id() == item_id) {
                *existing = item.clone();
            }
            let total = total_of_active_items(&all).map_err(sale_error)?;
            let previous = sale.revision();
            sale.recalculate_total(total).map_err(sale_error)?;
            if transaction
                .update_sale_cas(previous, &sale, occurred_at.as_str())
                .await
                .map_err(storage_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            transaction
                .remove_sale_item(&item, occurred_at.as_str())
                .await
                .map_err(storage_error)?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "sale.item.removed",
                "sale",
                sale.id().clone(),
                json!({"saleItemId": item_id.as_str()}),
                occurred_at,
            )
            .await?;
            Ok(sale)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Cancela una venta OPEN conservando líneas, snapshots y total histórico.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn cancel_sale<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: EntityId,
        cash_shift_id: EntityId,
        sale_id: &EntityId,
        expected_sale_version: u64,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Sale, ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleCancel,
            true,
            cash_register_id,
            cash_shift_id,
        )?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let mut sale = transaction
                .get_sale(&scope, sale_id)
                .await
                .map_err(storage_error)?
                .ok_or(ApplicationError::SaleNotFound)?;
            if sale.revision() != expected_sale_version {
                return Err(ApplicationError::ConcurrentModification);
            }
            let previous_total = sale.total_amount();
            let previous = sale.revision();
            sale.cancel(
                principal.user_id().clone(),
                transaction
                    .candidate_utc_seconds(occurred_at)
                    .await
                    .map_err(|_| ApplicationError::Storage)?,
            )
            .map_err(sale_error)?;
            if transaction
                .update_sale_cas(previous, &sale, occurred_at.as_str())
                .await
                .map_err(storage_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            append_audit(
                &mut transaction,
                identities,
                principal,
                "sale.cancelled",
                "sale",
                sale.id().clone(),
                json!({"previousTotal": previous_total.to_string()}),
                occurred_at,
            )
            .await?;
            Ok(sale)
        }
        .await;
        finish(transaction, operation).await
    }
}

impl SaleOperationalContext for SqliteTransaction {
    fn find_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Branch>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        async move {
            self.get_branch(&organization_id, &branch_id)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn find_cash_register(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashRegister>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let cash_register_id = cash_register_id.clone();
        async move {
            self.get_cash_register(&organization_id, &branch_id, &cash_register_id)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn find_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashShift>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let cash_register_id = cash_register_id.clone();
        let cash_shift_id = cash_shift_id.clone();
        async move {
            self.get_cash_shift(
                &organization_id,
                &branch_id,
                &cash_register_id,
                &cash_shift_id,
            )
            .await
            .map_err(|_| ApplicationError::Storage)
        }
    }
}

pub(crate) fn sale_error(error: SaleError) -> ApplicationError {
    match error {
        SaleError::SaleNotOpen => ApplicationError::SaleNotOpen,
        SaleError::SaleAlreadyCancelled => ApplicationError::SaleAlreadyCancelled,
        SaleError::SaleAlreadyCompleted => ApplicationError::SaleAlreadyCompleted,
        SaleError::ItemAlreadyRemoved => ApplicationError::SaleItemAlreadyRemoved,
        SaleError::ConcurrentModification => ApplicationError::ConcurrentModification,
        SaleError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        _ => ApplicationError::InvalidSaleInput,
    }
}
pub(crate) fn storage_error(error: SalesStorageError) -> ApplicationError {
    match error {
        SalesStorageError::ForeignKey => ApplicationError::OwnershipMismatch,
        _ => ApplicationError::Storage,
    }
}
pub(crate) fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}
pub(crate) async fn finish<T>(
    transaction: SqliteTransaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError> {
    match operation {
        Ok(value) => {
            transaction.commit().await.map_err(transaction_error)?;
            Ok(value)
        }
        Err(error) => {
            if transaction.rollback().await.is_err() {
                Err(ApplicationError::Storage)
            } else {
                Err(error)
            }
        }
    }
}
