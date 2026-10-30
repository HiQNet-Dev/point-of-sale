//! Casos de uso candidatos de ventas sobre SQLite real.
//!
//! No es código productivo: las tablas se aplican de modo explícito en pruebas
//! desde `proposals/local-sales/`. Cada mutación usa una sola UoW `BEGIN
//! IMMEDIATE`, conserva snapshots históricos y escribe auditoría antes del único
//! commit. Checkout no se expone aquí porque todavía no hay pagos e inventario
//! persistentes que puedan participar en su transacción atómica.

use std::{collections::BTreeMap, future::Future};

use pos_application::sales_context::SaleOperationalContext;
use pos_core::{
    audit::AuditOccurredAt,
    cash::{CashRegister, CashShift},
    catalog::{PriceDelta, UnitPrice},
    identity::UtcSeconds,
    organization_branch::{ApplicationError, Branch, EntityId, IdentityGenerator},
    sales::{
        ModifierGroupAssignment, Sale, SaleItem, SaleItemStatus, SaleScope, SaleStatus,
        SelectedModifierOption, resolve_modifier_pricing,
    },
};

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork, cash::OpenSalesForCashShift};

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

pub(crate) fn storage_error(error: SalesStorageError) -> ApplicationError {
    match error {
        SalesStorageError::ForeignKey => ApplicationError::OwnershipMismatch,
        _ => ApplicationError::Storage,
    }
}
pub(crate) fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}

pub use pos_application::sales::{
    AddSaleItemRequest, MAX_SALE_PAGE_SIZE, ModifierSnapshot, NewSaleRequest, SalePage,
    UpdateSaleItemRequest,
};

pub use pos_application::sales::{SalesApplication, SalesTransaction, SalesUnitOfWork, sale_error};

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteSalesApplication = SalesApplication<SqliteUnitOfWork>;

impl SalesUnitOfWork for SqliteUnitOfWork {
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

impl SalesTransaction for SqliteTransaction {
    fn get_sale(
        &mut self,
        scope: &SaleScope,
        sale_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Sale>, ApplicationError>> + Send {
        let scope = scope.clone();
        let sale_id = sale_id.clone();
        async move { self.get_sale(&scope, &sale_id).await.map_err(storage_error) }
    }

    fn list_sales(
        &mut self,
        scope: &SaleScope,
        status: Option<SaleStatus>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<Sale>, ApplicationError>> + Send {
        let scope = scope.clone();
        async move {
            self.list_sales(&scope, status, limit, offset)
                .await
                .map_err(storage_error)
        }
    }

    fn insert_sale(
        &mut self,
        sale: &Sale,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let sale = sale.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.insert_sale(&sale, &occurred_at)
                .await
                .map_err(storage_error)
        }
    }

    fn update_sale_cas(
        &mut self,
        previous: u64,
        sale: &Sale,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let sale = sale.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.update_sale_cas(previous, &sale, &occurred_at)
                .await
                .map_err(storage_error)
        }
    }

    fn get_sale_item(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
        item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<SaleItem>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let sale_id = sale_id.clone();
        let item_id = item_id.clone();
        async move {
            self.get_sale_item(&organization_id, &sale_id, &item_id)
                .await
                .map_err(storage_error)
        }
    }

    fn list_sale_items(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<SaleItem>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let sale_id = sale_id.clone();
        async move {
            self.list_sale_items(&organization_id, &sale_id)
                .await
                .map_err(storage_error)
        }
    }

    fn next_sale_item_position(
        &mut self,
        organization_id: &EntityId,
        sale_id: &EntityId,
    ) -> impl Future<Output = Result<u32, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let sale_id = sale_id.clone();
        async move {
            self.next_sale_item_position(&organization_id, &sale_id)
                .await
                .map_err(storage_error)
        }
    }

    fn insert_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let item = item.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.insert_sale_item(&item, &occurred_at)
                .await
                .map_err(storage_error)
        }
    }

    fn update_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let item = item.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.update_sale_item(&item, &occurred_at)
                .await
                .map_err(storage_error)
        }
    }

    fn remove_sale_item(
        &mut self,
        item: &SaleItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let item = item.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.remove_sale_item(&item, &occurred_at)
                .await
                .map_err(storage_error)
        }
    }

    fn item_option_ids(
        &mut self,
        item_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<EntityId>, ApplicationError>> + Send {
        let item_id = item_id.clone();
        async move { self.item_option_ids(&item_id).await.map_err(storage_error) }
    }

    fn replace_item_modifiers<I: IdentityGenerator + Send>(
        &mut self,
        identities: &mut I,
        item: &SaleItem,
        snapshots: &[ModifierSnapshot],
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let item = item.clone();
        let snapshots = snapshots.to_vec();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.replace_item_modifiers(identities, &item, &snapshots, &occurred_at)
                .await
        }
    }

    fn modifier_quote(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
        selected: &[EntityId],
        base: UnitPrice,
    ) -> impl Future<
        Output = Result<(PriceDelta, UnitPrice, Vec<ModifierSnapshot>), ApplicationError>,
    > + Send {
        let organization_id = organization_id.clone();
        let product_id = product_id.clone();
        let selected = selected.to_vec();
        async move {
            self.modifier_quote(&organization_id, &product_id, &selected, base)
                .await
        }
    }

    fn get_product(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> impl Future<Output = Result<Option<pos_core::catalog::Product>, ApplicationError>> + Send
    {
        let organization_id = organization_id.clone();
        let product_id = product_id.clone();
        async move {
            self.get_product(&organization_id, &product_id)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send {
        let occurred_at = occurred_at.clone();
        async move {
            self.candidate_utc_seconds(&occurred_at)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }
}
