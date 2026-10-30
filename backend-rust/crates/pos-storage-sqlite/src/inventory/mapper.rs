//! Mappers de fila del contexto.

use super::store::*;
use crate::prelude::*;

#[derive(sqlx::FromRow)]
pub(crate) struct InventoryItemRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) base_unit: String,
    pub(crate) fractional_unit_allowed: i64,
    pub(crate) unit_locked: i64,
    pub(crate) status: String,
    pub(crate) version: i64,
}

impl InventoryItemRow {
    pub(crate) fn into_domain(self) -> Result<InventoryItem, ApplicationError> {
        let description = match self.description {
            Some(value) => Description::new(&value).map_err(|_| storage())?,
            None => None,
        };
        Ok(InventoryItem::restore(
            EntityId::parse(&self.id).map_err(|_| storage())?,
            EntityId::parse(&self.organization_id).map_err(|_| storage())?,
            CatalogCode::restore(&self.code).map_err(|_| storage())?,
            DisplayName::new(&self.name).map_err(|_| storage())?,
            description,
            InventoryBaseUnit::parse(&self.base_unit).map_err(|_| storage())?,
            self.fractional_unit_allowed != 0,
            self.unit_locked != 0,
            Status::parse(&self.status).map_err(|_| storage())?,
            u64::try_from(self.version).map_err(|_| storage())?,
        ))
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct StockBalanceRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) branch_id: String,
    pub(crate) inventory_item_id: String,
    pub(crate) quantity_on_hand: String,
    pub(crate) version: i64,
}

impl StockBalanceRow {
    pub(crate) fn into_domain(self) -> Result<StockBalance, ApplicationError> {
        Ok(StockBalance::restore(
            EntityId::parse(&self.id).map_err(|_| storage())?,
            EntityId::parse(&self.organization_id).map_err(|_| storage())?,
            EntityId::parse(&self.branch_id).map_err(|_| storage())?,
            EntityId::parse(&self.inventory_item_id).map_err(|_| storage())?,
            Quantity::parse(&self.quantity_on_hand).map_err(|_| storage())?,
            u64::try_from(self.version).map_err(|_| storage())?,
        ))
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct MovementRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) branch_id: String,
    pub(crate) inventory_item_id: String,
    pub(crate) movement_type: String,
    pub(crate) quantity_delta: String,
    pub(crate) resulting_quantity: String,
    pub(crate) reason: Option<String>,
    pub(crate) actor_user_id: String,
    pub(crate) occurred_at: i64,
    pub(crate) sale_id: Option<String>,
    pub(crate) purchase_receipt_line_id: Option<String>,
}

impl MovementRow {
    pub(crate) fn into_domain(self) -> Result<InventoryMovement, ApplicationError> {
        let reason = match self.reason {
            Some(value) => Some(MovementReason::restore(&value).map_err(|_| storage())?),
            None => None,
        };
        InventoryMovement::restore(
            EntityId::parse(&self.id).map_err(|_| storage())?,
            EntityId::parse(&self.organization_id).map_err(|_| storage())?,
            EntityId::parse(&self.branch_id).map_err(|_| storage())?,
            EntityId::parse(&self.inventory_item_id).map_err(|_| storage())?,
            InventoryMovementType::parse(&self.movement_type).map_err(|_| storage())?,
            Quantity::parse(&self.quantity_delta).map_err(|_| storage())?,
            Quantity::parse(&self.resulting_quantity).map_err(|_| storage())?,
            reason,
            EntityId::parse(&self.actor_user_id).map_err(|_| storage())?,
            UtcSeconds::new(self.occurred_at).map_err(|_| storage())?,
            match self.sale_id {
                Some(value) => Some(EntityId::parse(&value).map_err(|_| storage())?),
                None => None,
            },
            match self.purchase_receipt_line_id {
                Some(value) => Some(EntityId::parse(&value).map_err(|_| storage())?),
                None => None,
            },
        )
        .map_err(|_| storage())
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct RecipeRow {
    pub(crate) recipe_status: String,
    pub(crate) inventory_item_id: Option<String>,
    pub(crate) quantity_per_product: Option<String>,
    pub(crate) base_unit: Option<String>,
    pub(crate) fractional_unit_allowed: Option<i64>,
    pub(crate) active: Option<i64>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct EffectRow {
    pub(crate) inventory_item_id: String,
    pub(crate) base_unit: String,
    pub(crate) fractional_unit_allowed: i64,
    pub(crate) active: i64,
    pub(crate) quantity_delta: String,
}
