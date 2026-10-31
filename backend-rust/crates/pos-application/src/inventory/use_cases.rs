//! Casos de uso de inventario sobre puertos de aplicación.

use pos_core::{
    audit::AuditOccurredAt,
    catalog::{CatalogCode, Description},
    identity::VerifiedPrincipal,
    inventory::{
        InventoryError, InventoryItem, InventoryMovement, MovementReason, NewInventoryItem,
        NewMovement, Quantity, StockBalance, ensure_opening_allowed,
    },
    organization_branch::Permission,
    organization_branch::{ApplicationError, DisplayName, EntityId, IdentityGenerator},
};
use serde_json::json;

use crate::{
    CasOutcome,
    audit::{AuditAppender, append_audit},
    authorization::{require_operational_branch, require_organization_scope},
    inventory::{dto::*, ports::*},
};

/// Casos de uso candidatos de inventario. No forma parte del composition root
/// productivo mientras no exista DDL aprobado.
#[derive(Clone)]
pub struct InventoryApplication<U> {
    unit_of_work: U,
}

impl<U> InventoryApplication<U>
where
    U: InventoryUnitOfWork,
    U::Transaction: AuditAppender + Send,
{
    /// Ensambla inventario candidato sobre una UoW SQLite configurada.
    pub fn new(unit_of_work: U) -> Self {
        Self { unit_of_work }
    }

    /// Crea un artículo activo; el código es único por organización.
    pub async fn create_item<I: IdentityGenerator + Send>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewInventoryItemRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<InventoryItem, ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::InventoryItemWrite)?;
        let code = CatalogCode::new(&request.code).map_err(|_| invalid())?;
        let name = DisplayName::new(&request.name).map_err(|_| invalid())?;
        let description = match request.description.as_deref() {
            Some(value) => Description::new(value).map_err(|_| invalid())?,
            None => None,
        };
        let item = InventoryItem::create(NewInventoryItem {
            id: identities.next_id()?,
            organization_id: organization_id.clone(),
            code,
            name,
            description,
            base_unit: request.base_unit,
            fractional_unit_allowed: request.fractional_unit_allowed,
        });
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            if transaction
                .inventory_item_code_exists(item.organization_id(), item.code())
                .await?
            {
                return Err(ApplicationError::InventoryItemCodeAlreadyExists);
            }
            transaction
                .insert_inventory_item(&item, occurred_at.as_str())
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "inventory.item.created",
                "inventory-item",
                item.id().clone(),
                json!({"code": item.code().as_str(), "baseUnit": item.base_unit().as_str()}),
                occurred_at,
            )
            .await?;
            Ok(item)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Lee un artículo dentro de la organización del principal.
    pub async fn get_item(
        &self,
        principal: &VerifiedPrincipal,
        inventory_item_id: &EntityId,
    ) -> Result<InventoryItem, ApplicationError> {
        let organization_id = require_organization_scope(principal, Permission::InventoryItemRead)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            transaction
                .get_inventory_item(&organization_id, inventory_item_id)
                .await?
                .ok_or(ApplicationError::InventoryItemNotFound)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Lista artículos de la organización en orden estable.
    pub async fn list_items(
        &self,
        principal: &VerifiedPrincipal,
        page: InventoryPage,
    ) -> Result<Vec<InventoryItem>, ApplicationError> {
        let organization_id = require_organization_scope(principal, Permission::InventoryItemRead)?;
        if page.limit == 0 || page.limit > MAX_INVENTORY_PAGE_SIZE {
            return Err(invalid());
        }
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            transaction
                .list_inventory_items(&organization_id, limit, offset)
                .await
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Registra la apertura única de existencias de un artículo en la sucursal
    /// del principal verificado.
    pub async fn open_stock<I: IdentityGenerator + Send>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: OpenStockRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<(StockBalance, InventoryMovement), ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::InventoryStockAdjust)?;
        let branch_id = require_operational_branch(principal)?;
        let quantity = Quantity::parse(&request.quantity).map_err(|_| invalid())?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let item = transaction
                .get_inventory_item(&organization_id, &request.inventory_item_id)
                .await?
                .ok_or(ApplicationError::InventoryItemNotFound)?;
            if !item.is_active() {
                return Err(ApplicationError::InventoryItemInactive);
            }
            item.ensure_quantity_fits_unit(quantity)
                .map_err(inventory_error)?;
            let (has_opening, has_any) = transaction
                .ledger_counts(&organization_id, &branch_id, item.id())
                .await?;
            ensure_opening_allowed(has_opening, has_any).map_err(inventory_error)?;

            let existing = transaction
                .get_stock_balance(&organization_id, &branch_id, item.id())
                .await?;
            let (mut balance, balance_was_missing) = match existing {
                Some(balance) => (balance, false),
                None => (
                    StockBalance::empty(
                        identities.next_id()?,
                        organization_id.clone(),
                        branch_id.clone(),
                        item.id().clone(),
                    ),
                    true,
                ),
            };
            let previous_revision = balance.revision();
            let resulting = balance
                .apply_delta(previous_revision, quantity)
                .map_err(inventory_error)?;
            if balance_was_missing {
                transaction
                    .insert_stock_balance(&balance, occurred_at.as_str())
                    .await?;
            } else {
                match transaction
                    .update_stock_balance_cas(previous_revision, &balance, occurred_at.as_str())
                    .await?
                {
                    CasOutcome::Updated => {}
                    CasOutcome::Missing | CasOutcome::Conflict => {
                        return Err(ApplicationError::ConcurrentModification);
                    }
                }
            }

            let movement = InventoryMovement::opening(
                NewMovement {
                    id: identities.next_id()?,
                    organization_id: organization_id.clone(),
                    branch_id: branch_id.clone(),
                    inventory_item_id: item.id().clone(),
                    actor_user_id: principal.user_id().clone(),
                    occurred_at: transaction
                        .candidate_utc_seconds(occurred_at)
                        .await
                        .map_err(|_| storage())?,
                },
                quantity,
                resulting,
            )
            .map_err(inventory_error)?;
            transaction.insert_inventory_movement(&movement).await?;
            transaction
                .lock_inventory_unit(&organization_id, item.id(), occurred_at.as_str())
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "inventory.stock.opened",
                "inventory-item",
                item.id().clone(),
                json!({
                    "branchId": branch_id.as_str(),
                    "quantity": quantity.to_string(),
                    "resulting": resulting.to_string(),
                }),
                occurred_at,
            )
            .await?;
            Ok((balance, movement))
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Aplica un ajuste manual sobre un saldo previamente aperturado.
    pub async fn adjust_stock<I: IdentityGenerator + Send>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: AdjustStockRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<(StockBalance, InventoryMovement), ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::InventoryStockAdjust)?;
        let branch_id = require_operational_branch(principal)?;
        let delta = Quantity::parse(&request.quantity_delta).map_err(|_| invalid())?;
        let reason = MovementReason::new(&request.reason).map_err(inventory_error)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let item = transaction
                .get_inventory_item(&organization_id, &request.inventory_item_id)
                .await?
                .ok_or(ApplicationError::InventoryItemNotFound)?;
            if !item.is_active() {
                return Err(ApplicationError::InventoryItemInactive);
            }
            item.ensure_quantity_fits_unit(delta)
                .map_err(inventory_error)?;
            let mut balance = transaction
                .get_stock_balance(&organization_id, &branch_id, item.id())
                .await?
                .ok_or(ApplicationError::InvalidInventoryInput)?;
            let previous_revision = balance.revision();
            let resulting = balance
                .apply_delta(previous_revision, delta)
                .map_err(inventory_error)?;
            if transaction
                .update_stock_balance_cas(previous_revision, &balance, occurred_at.as_str())
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            let movement = InventoryMovement::adjustment(
                NewMovement {
                    id: identities.next_id()?,
                    organization_id: organization_id.clone(),
                    branch_id: branch_id.clone(),
                    inventory_item_id: item.id().clone(),
                    actor_user_id: principal.user_id().clone(),
                    occurred_at: transaction
                        .candidate_utc_seconds(occurred_at)
                        .await
                        .map_err(|_| storage())?,
                },
                delta,
                resulting,
                reason,
            )
            .map_err(inventory_error)?;
            transaction.insert_inventory_movement(&movement).await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "inventory.stock.adjusted",
                "inventory-item",
                item.id().clone(),
                json!({
                    "branchId": branch_id.as_str(),
                    "delta": delta.to_string(),
                    "resulting": resulting.to_string(),
                }),
                occurred_at,
            )
            .await?;
            Ok((balance, movement))
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Lee el saldo de un artículo en la sucursal del principal.
    pub async fn get_balance(
        &self,
        principal: &VerifiedPrincipal,
        inventory_item_id: &EntityId,
    ) -> Result<Option<StockBalance>, ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::InventoryStockRead)?;
        let branch_id = require_operational_branch(principal)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            transaction
                .get_stock_balance(&organization_id, &branch_id, inventory_item_id)
                .await
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Lista el ledger de un artículo en orden histórico descendente.
    pub async fn list_movements(
        &self,
        principal: &VerifiedPrincipal,
        inventory_item_id: &EntityId,
        page: InventoryPage,
    ) -> Result<Vec<InventoryMovement>, ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::InventoryStockRead)?;
        let branch_id = require_operational_branch(principal)?;
        if page.limit == 0 || page.limit > MAX_INVENTORY_PAGE_SIZE {
            return Err(invalid());
        }
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            transaction
                .list_inventory_movements(
                    &organization_id,
                    &branch_id,
                    inventory_item_id,
                    limit,
                    offset,
                )
                .await
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }
}

fn storage() -> ApplicationError {
    ApplicationError::Storage
}

fn invalid() -> ApplicationError {
    ApplicationError::InvalidInventoryInput
}

/// Traduce un fallo de dominio sin perder el tipado público.
fn inventory_error(error: InventoryError) -> ApplicationError {
    match error {
        InventoryError::InsufficientInventory => ApplicationError::InsufficientInventory,
        InventoryError::OpeningAlreadyExists => ApplicationError::InventoryOpeningAlreadyExists,
        InventoryError::ConcurrentModification => ApplicationError::ConcurrentModification,
        InventoryError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        InventoryError::InvalidQuantity
        | InventoryError::QuantityOverflow
        | InventoryError::UnitLocked
        | InventoryError::FractionalQuantityNotAllowed
        | InventoryError::ReasonRequired
        | InventoryError::ReasonTooLong
        | InventoryError::HistoryAlreadyExists
        | InventoryError::OpeningMustBeNonNegative
        | InventoryError::AdjustmentMustNotBeZero
        | InventoryError::SaleConsumptionMustBeNegative
        | InventoryError::SaleConsumptionRequiresSale
        | InventoryError::PurchaseReceiptMustBePositive
        | InventoryError::PurchaseReceiptRequiresLine
        | InventoryError::UnexpectedMovementReference => ApplicationError::InvalidInventoryInput,
        InventoryError::RevisionOverflow | InventoryError::InvalidPersistedValue => storage(),
    }
}

/// Confirma o revierte a través de la unidad de trabajo.
async fn finish<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: InventoryUnitOfWork,
{
    match operation {
        Ok(value) => {
            unit_of_work.commit_transaction(transaction).await?;
            Ok(value)
        }
        Err(error) => {
            unit_of_work.rollback_transaction(transaction).await?;
            Err(error)
        }
    }
}
