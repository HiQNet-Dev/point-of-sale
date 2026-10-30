//! Implementación del repositorio sobre la transacción SQLite.

use std::collections::BTreeMap;

use pos_application::sales_context::SaleOperationalContext;

use super::mapper::*;
use super::queries::*;
use super::store::*;
use crate::prelude::*;

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
