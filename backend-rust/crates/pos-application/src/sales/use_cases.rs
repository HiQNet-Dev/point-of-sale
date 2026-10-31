//! Casos de uso de ventas sobre puertos de aplicación.

use pos_core::{
    audit::AuditOccurredAt,
    identity::VerifiedPrincipal,
    organization_branch::{ApplicationError, EntityId, IdentityGenerator, Permission, Status},
    sales::{NewSale, NewSaleItem, Sale, SaleError, SaleItem, SaleStatus, total_of_active_items},
};
use serde_json::json;

use crate::{
    CasOutcome,
    audit::{AuditAppender, append_audit},
    authorization::require_sale_scope,
    sales::{dto::*, ports::*},
    sales_context::{SaleOperationalContext, require_open_operational_shift},
};

/// Casos de uso candidatos de ventas. No ofrece checkout hasta que pagos e inventario sean atómicos.
#[derive(Clone)]
pub struct SalesApplication<U> {
    unit_of_work: U,
}

impl<U> SalesApplication<U>
where
    U: SalesUnitOfWork,
    U::Transaction: AuditAppender + SaleOperationalContext + Send,
{
    /// Ensambla ventas candidatas sobre una UoW SQLite configurada.
    pub fn new(unit_of_work: U) -> Self {
        Self { unit_of_work }
    }

    /// Abre una venta solo cuando sucursal, caja y turno están activos/abiertos.
    pub async fn create_sale<I: IdentityGenerator + Send>(
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let sale = Sale::open(NewSale { id: sale_id, scope: scope.clone(), created_by_user_id: principal.user_id().clone() });
            transaction.insert_sale(&sale, occurred_at.as_str()).await?;
            append_audit(&mut transaction, identities, principal, "sale.created", "sale", sale.id().clone(), json!({"cashRegisterId": scope.cash_register_id.as_str(), "cashShiftId": scope.cash_shift_id.as_str()}), occurred_at).await?;
            Ok(sale)
        }.await;
        finish(&self.unit_of_work, transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let result = async {
            let sale = transaction
                .get_sale(&scope, sale_id)
                .await?
                .ok_or(ApplicationError::SaleNotFound)?;
            let items = transaction
                .list_sale_items(&scope.organization_id, sale_id)
                .await?;
            Ok((sale, items))
        }
        .await;
        finish(&self.unit_of_work, transaction, result).await
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let result = transaction
            .list_sales(
                &scope,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await;
        finish(&self.unit_of_work, transaction, result).await
    }

    /// Cotiza, persiste y audita una partida; actualiza el total de venta mediante CAS.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_sale_item<I: IdentityGenerator + Send>(
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let mut sale = transaction.get_sale(&scope, sale_id).await?.ok_or(ApplicationError::SaleNotFound)?;
            sale.require_open().map_err(sale_error)?;
            if sale.revision() != request.expected_sale_version { return Err(ApplicationError::ConcurrentModification); }
            let product = transaction.get_product(&scope.organization_id, &request.product_id).await.map_err(|_| ApplicationError::Storage)?.ok_or(ApplicationError::ProductNotFound)?;
            if product.status() != Status::Active { return Err(ApplicationError::ProductInactive); }
            let (delta, effective, snapshots) = transaction.modifier_quote(&scope.organization_id, product.id(), &request.modifier_option_ids, product.unit_price()).await?;
            let position = transaction.next_sale_item_position(&scope.organization_id, sale_id).await?;
            let item = SaleItem::create(NewSaleItem { id: item_id, organization_id: scope.organization_id.clone(), sale_id: sale_id.clone(), product_id: product.id().clone(), product_sku_snapshot: product.sku().clone(), product_name_snapshot: product.name().clone(), base_unit_price: product.unit_price(), modifier_unit_delta: delta, effective_unit_price: effective, quantity: request.quantity, position }).map_err(sale_error)?;
            let mut all = transaction.list_sale_items(&scope.organization_id, sale_id).await?;
            all.push(item.clone());
            let total = total_of_active_items(&all).map_err(sale_error)?;
            let previous = sale.revision();
            sale.recalculate_total(total).map_err(sale_error)?;
            if transaction.update_sale_cas(previous, &sale, occurred_at.as_str()).await? != CasOutcome::Updated { return Err(ApplicationError::ConcurrentModification); }
            transaction.insert_sale_item(&item, occurred_at.as_str()).await?;
            transaction.replace_item_modifiers(identities, &item, &snapshots, occurred_at.as_str()).await?;
            append_audit(&mut transaction, identities, principal, "sale.item.added", "sale", sale.id().clone(), json!({"productId": product.id().as_str(), "quantity": item.quantity(), "lineTotal": item.line_total().to_string()}), occurred_at).await?;
            Ok((sale, item))
        }.await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Recotiza una partida activa y reemplaza sus snapshots en la misma UoW.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_sale_item<I: IdentityGenerator + Send>(
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let mut sale = transaction.get_sale(&scope, sale_id).await?.ok_or(ApplicationError::SaleNotFound)?;
            sale.require_open().map_err(sale_error)?;
            if sale.revision() != request.expected_sale_version { return Err(ApplicationError::ConcurrentModification); }
            let mut item = transaction.get_sale_item(&scope.organization_id, sale_id, item_id).await?.ok_or(ApplicationError::SaleItemNotFound)?;
            item.require_active().map_err(sale_error)?;
            let product = transaction.get_product(&scope.organization_id, item.product_id()).await.map_err(|_| ApplicationError::Storage)?.ok_or(ApplicationError::ProductNotFound)?;
            if product.status() != Status::Active { return Err(ApplicationError::ProductInactive); }
            let (delta, effective, snapshots) = transaction.modifier_quote(&scope.organization_id, product.id(), &request.modifier_option_ids, product.unit_price()).await?;
            let previous_options = transaction.item_option_ids(item_id).await?;
            let mut requested_options = request.modifier_option_ids.clone();
            requested_options.sort();
            let same_options = previous_options == requested_options;
            if item.quantity() == request.quantity && same_options { return Ok((sale, item)); }
            item.update_quote(product.unit_price(), delta, effective, request.quantity).map_err(sale_error)?;
            let mut all = transaction.list_sale_items(&scope.organization_id, sale_id).await?;
            if let Some(existing) = all.iter_mut().find(|existing| existing.id() == item_id) { *existing = item.clone(); } else { return Err(ApplicationError::SaleItemNotFound); }
            let total = total_of_active_items(&all).map_err(sale_error)?;
            let previous = sale.revision(); sale.recalculate_total(total).map_err(sale_error)?;
            if transaction.update_sale_cas(previous, &sale, occurred_at.as_str()).await? != CasOutcome::Updated { return Err(ApplicationError::ConcurrentModification); }
            transaction.update_sale_item(&item, occurred_at.as_str()).await?;
            transaction.replace_item_modifiers(identities, &item, &snapshots, occurred_at.as_str()).await?;
            append_audit(&mut transaction, identities, principal, "sale.item.updated", "sale", sale.id().clone(), json!({"saleItemId": item_id.as_str(), "changedFields": ["quantity", "modifierOptionIds"]}), occurred_at).await?;
            Ok((sale, item))
        }.await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Retira lógicamente una partida activa y recalcula el total con CAS.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn remove_sale_item<I: IdentityGenerator + Send>(
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            require_open_operational_shift(&mut transaction, &scope).await?;
            let mut sale = transaction
                .get_sale(&scope, sale_id)
                .await?
                .ok_or(ApplicationError::SaleNotFound)?;
            sale.require_open().map_err(sale_error)?;
            if sale.revision() != expected_sale_version {
                return Err(ApplicationError::ConcurrentModification);
            }
            let mut item = transaction
                .get_sale_item(&scope.organization_id, sale_id, item_id)
                .await?
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
                .await?;
            if let Some(existing) = all.iter_mut().find(|existing| existing.id() == item_id) {
                *existing = item.clone();
            }
            let total = total_of_active_items(&all).map_err(sale_error)?;
            let previous = sale.revision();
            sale.recalculate_total(total).map_err(sale_error)?;
            if transaction
                .update_sale_cas(previous, &sale, occurred_at.as_str())
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            transaction
                .remove_sale_item(&item, occurred_at.as_str())
                .await?;
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
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Cancela una venta OPEN conservando líneas, snapshots y total histórico.
    ///
    /// Los IDs de scope permanecen explícitos para no aceptar owner ni operador desde el cliente.
    #[allow(clippy::too_many_arguments)]
    pub async fn cancel_sale<I: IdentityGenerator + Send>(
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let mut sale = transaction
                .get_sale(&scope, sale_id)
                .await?
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
                .await?
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
        finish(&self.unit_of_work, transaction, operation).await
    }
}

/// Confirma o revierte a través de la unidad de trabajo.
async fn finish<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: SalesUnitOfWork,
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

/// Traduce un fallo de dominio de ventas al error de aplicación.
pub fn sale_error(error: SaleError) -> ApplicationError {
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
