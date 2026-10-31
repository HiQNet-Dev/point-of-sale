//! Mappers de fila del contexto.

use super::store::*;
use crate::prelude::*;

#[derive(sqlx::FromRow)]
pub(crate) struct SaleRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) branch_id: String,
    pub(crate) cash_register_id: String,
    pub(crate) cash_shift_id: String,
    pub(crate) created_by_user_id: String,
    pub(crate) cancelled_by_user_id: Option<String>,
    pub(crate) completed_by_user_id: Option<String>,
    pub(crate) status: String,
    pub(crate) total_amount: String,
    pub(crate) version: i64,
    pub(crate) cancelled_at: Option<i64>,
    pub(crate) completed_at: Option<i64>,
}

impl SaleRow {
    pub(crate) fn into_domain(self) -> Result<Sale, SalesStorageError> {
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
pub(crate) struct SaleItemRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) sale_id: String,
    pub(crate) product_id: String,
    pub(crate) product_sku_snapshot: String,
    pub(crate) product_name_snapshot: String,
    pub(crate) base_unit_price: String,
    pub(crate) modifier_unit_delta: String,
    pub(crate) effective_unit_price: String,
    pub(crate) quantity: i64,
    pub(crate) line_total: String,
    pub(crate) status: String,
    pub(crate) position: i64,
    pub(crate) removed_by_user_id: Option<String>,
    pub(crate) removed_at: Option<i64>,
}

impl SaleItemRow {
    pub(crate) fn into_domain(self) -> Result<SaleItem, SalesStorageError> {
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
pub(crate) struct ModifierConfigurationRow {
    pub(crate) modifier_group_id: String,

    pub(crate) group_code: String,
    pub(crate) group_name: String,
    pub(crate) group_status: String,
    pub(crate) min_selections: i64,
    pub(crate) max_selections: i64,
    pub(crate) option_id: Option<String>,
    pub(crate) option_code: Option<String>,
    pub(crate) option_name: Option<String>,
    pub(crate) option_status: Option<String>,
    pub(crate) price_delta: Option<String>,
    pub(crate) option_display_order: Option<i64>,
}
