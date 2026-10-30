//! Casos de uso candidatos de inventario sobre SQLite real.
//!
//! **No productivo.** Las tablas viven en `proposals/local-inventory/` y las
//! pruebas las aplican de forma explícita; `pos-local::migrate()` no descubre
//! esos archivos. Cada mutación comparte una única UoW `BEGIN IMMEDIATE` con el
//! ledger y la auditoría, y confirma solo si ambos se escribieron.
//!
//! # Invariantes
//!
//! - La apertura es única por sucursal y artículo y precede a cualquier otro
//!   movimiento ([`ensure_opening_allowed`]).
//! - Los ajustes exigen un saldo aperturado y un motivo normalizado.
//! - Las cantidades viajan como `TEXT` decimal canónico escala 6; nunca se usa
//!   `REAL` ni aritmética de punto flotante.
//! - La unidad base se congela en el primer movimiento y no vuelve a liberarse.
//! - El ledger es append-only por trigger; el saldo se materializa por CAS.
//! - La sucursal siempre procede del principal verificado, nunca del cliente.

use pos_core::{
    audit::AuditOccurredAt,
    catalog::{CatalogCode, Description},
    identity::UtcSeconds,
    inventory::{
        InventoryBaseUnit, InventoryItem, InventoryMovement, InventoryMovementType, MovementReason,
        Quantity, StockBalance,
    },
    organization_branch::{ApplicationError, DisplayName, EntityId, Status},
    recipe::{ResolvedComponent, ResolvedEffect},
};

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork};

const ITEM_CODE_EXISTS: &str =
    "SELECT EXISTS(SELECT 1 FROM inventory_item WHERE organization_id = ? AND code = ?)";

fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}

fn revision_to_i64(value: u64) -> Result<i64, ApplicationError> {
    i64::try_from(value).map_err(|_| storage())
}

fn map_inventory_write(error: &sqlx::Error) -> ApplicationError {
    let Some(database) = error.as_database_error() else {
        return storage();
    };
    if database.is_foreign_key_violation() {
        ApplicationError::OwnershipMismatch
    } else if database.is_check_violation() || database.is_unique_violation() {
        ApplicationError::InvalidInventoryInput
    } else {
        storage()
    }
}

#[derive(sqlx::FromRow)]
struct InventoryItemRow {
    id: String,
    organization_id: String,
    code: String,
    name: String,
    description: Option<String>,
    base_unit: String,
    fractional_unit_allowed: i64,
    unit_locked: i64,
    status: String,
    version: i64,
}

impl InventoryItemRow {
    fn into_domain(self) -> Result<InventoryItem, ApplicationError> {
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
struct StockBalanceRow {
    id: String,
    organization_id: String,
    branch_id: String,
    inventory_item_id: String,
    quantity_on_hand: String,
    version: i64,
}

impl StockBalanceRow {
    fn into_domain(self) -> Result<StockBalance, ApplicationError> {
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
struct MovementRow {
    id: String,
    organization_id: String,
    branch_id: String,
    inventory_item_id: String,
    movement_type: String,
    quantity_delta: String,
    resulting_quantity: String,
    reason: Option<String>,
    actor_user_id: String,
    occurred_at: i64,
    sale_id: Option<String>,
    purchase_receipt_line_id: Option<String>,
}

impl MovementRow {
    fn into_domain(self) -> Result<InventoryMovement, ApplicationError> {
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
struct RecipeRow {
    recipe_status: String,
    inventory_item_id: Option<String>,
    quantity_per_product: Option<String>,
    base_unit: Option<String>,
    fractional_unit_allowed: Option<i64>,
    active: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct EffectRow {
    inventory_item_id: String,
    base_unit: String,
    fractional_unit_allowed: i64,
    active: i64,
    quantity_delta: String,
}

impl SqliteTransaction {
    /// Busca un artículo exclusivamente dentro de su organización.
    pub(crate) async fn get_inventory_item(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> Result<Option<InventoryItem>, ApplicationError> {
        let row = sqlx::query_as::<_, InventoryItemRow>(SELECT_ITEM)
            .bind(organization_id.as_str())
            .bind(inventory_item_id.as_str())
            .fetch_optional(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        row.map(InventoryItemRow::into_domain).transpose()
    }

    /// Indica si el código ya está ocupado dentro de la organización.
    pub(crate) async fn inventory_item_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> Result<bool, ApplicationError> {
        let exists: i64 = sqlx::query_scalar(ITEM_CODE_EXISTS)
            .bind(organization_id.as_str())
            .bind(code.as_str())
            .fetch_one(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        Ok(exists != 0)
    }

    /// Inserta un artículo nuevo con revisión cero.
    pub(crate) async fn insert_inventory_item(
        &mut self,
        item: &InventoryItem,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(INSERT_ITEM)
            .bind(item.id().as_str())
            .bind(item.organization_id().as_str())
            .bind(item.code().as_str())
            .bind(item.name().as_str())
            .bind(item.description().map(Description::as_str))
            .bind(item.base_unit().as_str())
            .bind(i64::from(item.fractional_unit_allowed()))
            .bind(item.status().as_str())
            .bind(revision_to_i64(item.revision())?)
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.connection())
            .await
            .map_err(|error| map_inventory_write(&error))?;
        Ok(())
    }

    /// Congela la unidad base tras el primer movimiento.
    pub(crate) async fn lock_inventory_unit(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(LOCK_UNIT)
            .bind(occurred_at)
            .bind(organization_id.as_str())
            .bind(inventory_item_id.as_str())
            .execute(&mut *self.connection())
            .await
            .map_err(|error| map_inventory_write(&error))?;
        Ok(())
    }

    /// Lee el saldo materializado o `None` cuando la fila aún no existe.
    pub(crate) async fn get_stock_balance(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> Result<Option<StockBalance>, ApplicationError> {
        let row = sqlx::query_as::<_, StockBalanceRow>(SELECT_BALANCE)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(inventory_item_id.as_str())
            .fetch_optional(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        row.map(StockBalanceRow::into_domain).transpose()
    }

    /// Inserta el saldo de la primera apertura.
    pub(crate) async fn insert_stock_balance(
        &mut self,
        balance: &StockBalance,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(INSERT_BALANCE)
            .bind(balance.id().as_str())
            .bind(balance.organization_id().as_str())
            .bind(balance.branch_id().as_str())
            .bind(balance.inventory_item_id().as_str())
            .bind(balance.quantity_on_hand().to_string())
            .bind(revision_to_i64(balance.revision())?)
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.connection())
            .await
            .map_err(|error| map_inventory_write(&error))?;
        Ok(())
    }

    /// Actualiza el saldo solo si la revisión previa coincide.
    pub(crate) async fn update_stock_balance_cas(
        &mut self,
        previous_revision: u64,
        balance: &StockBalance,
        occurred_at: &str,
    ) -> Result<CasOutcome, ApplicationError> {
        let result = sqlx::query(UPDATE_BALANCE_CAS)
            .bind(balance.quantity_on_hand().to_string())
            .bind(revision_to_i64(balance.revision())?)
            .bind(occurred_at)
            .bind(balance.organization_id().as_str())
            .bind(balance.branch_id().as_str())
            .bind(balance.inventory_item_id().as_str())
            .bind(revision_to_i64(previous_revision)?)
            .execute(&mut *self.connection())
            .await
            .map_err(|error| map_inventory_write(&error))?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let exists: i64 = sqlx::query_scalar(BALANCE_EXISTS)
            .bind(balance.organization_id().as_str())
            .bind(balance.branch_id().as_str())
            .bind(balance.inventory_item_id().as_str())
            .fetch_one(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        Ok(if exists == 0 {
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
        })
    }

    /// Indica si existe apertura y si existe cualquier movimiento.
    async fn ledger_counts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> Result<(bool, bool), ApplicationError> {
        let row: (i64, i64) = sqlx::query_as(LEDGER_COUNTS)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(inventory_item_id.as_str())
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(inventory_item_id.as_str())
            .fetch_one(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        Ok((row.0 != 0, row.1 != 0))
    }

    /// Añade un hecho inmutable al ledger.
    pub(crate) async fn insert_inventory_movement(
        &mut self,
        movement: &InventoryMovement,
    ) -> Result<(), ApplicationError> {
        sqlx::query(INSERT_MOVEMENT)
            .bind(movement.id().as_str())
            .bind(movement.organization_id().as_str())
            .bind(movement.branch_id().as_str())
            .bind(movement.inventory_item_id().as_str())
            .bind(movement.movement_type().as_str())
            .bind(movement.quantity_delta().to_string())
            .bind(movement.resulting_quantity().to_string())
            .bind(movement.reason().map(MovementReason::as_str))
            .bind(movement.actor_user_id().as_str())
            .bind(movement.occurred_at().get())
            .bind(movement.sale_id().map(EntityId::as_str))
            .bind(movement.purchase_receipt_line_id().map(EntityId::as_str))
            .execute(&mut *self.connection())
            .await
            .map_err(|error| map_inventory_write(&error))?;
        Ok(())
    }

    async fn list_inventory_movements(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InventoryMovement>, ApplicationError> {
        let rows = sqlx::query_as::<_, MovementRow>(SELECT_MOVEMENTS)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(inventory_item_id.as_str())
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        rows.into_iter().map(MovementRow::into_domain).collect()
    }

    /// Resuelve la receta vigente de un producto o `None` si no está trackeado.
    ///
    /// Un producto sin receta activa no consume inventario; una receta ACTIVE sin
    /// componentes es una fila incoherente y se rechaza en vez de ignorarse.
    pub(crate) async fn active_recipe(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> Result<Option<Vec<ResolvedComponent>>, ApplicationError> {
        let rows = sqlx::query_as::<_, RecipeRow>(SELECT_ACTIVE_RECIPE)
            .bind(organization_id.as_str())
            .bind(product_id.as_str())
            .fetch_all(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        let Some(first) = rows.first() else {
            return Ok(None);
        };
        if first.recipe_status != "ACTIVE" {
            return Ok(None);
        }
        let mut components = Vec::with_capacity(rows.len());
        for row in rows {
            let Some(inventory_item_id) = row.inventory_item_id else {
                return Err(storage());
            };
            let Some(quantity) = row.quantity_per_product else {
                return Err(storage());
            };
            let Some(base_unit) = row.base_unit else {
                return Err(storage());
            };
            components.push(ResolvedComponent {
                inventory_item_id: EntityId::parse(&inventory_item_id).map_err(|_| storage())?,
                base_unit: InventoryBaseUnit::parse(&base_unit).map_err(|_| storage())?,
                fractional_unit_allowed: row.fractional_unit_allowed.unwrap_or_default() != 0,
                active: row.active.unwrap_or_default() != 0,
                quantity_per_product: Quantity::parse(&quantity).map_err(|_| storage())?,
            });
        }
        if components.is_empty() {
            return Err(storage());
        }
        Ok(Some(components))
    }

    /// Efectos de inventario de las opciones seleccionadas en una línea.
    pub(crate) async fn sale_item_modifier_effects(
        &mut self,
        organization_id: &EntityId,
        sale_item_id: &EntityId,
    ) -> Result<Vec<ResolvedEffect>, ApplicationError> {
        let rows = sqlx::query_as::<_, EffectRow>(SELECT_ITEM_EFFECTS)
            .bind(organization_id.as_str())
            .bind(sale_item_id.as_str())
            .fetch_all(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        rows.into_iter()
            .map(|row| {
                Ok(ResolvedEffect {
                    inventory_item_id: EntityId::parse(&row.inventory_item_id)
                        .map_err(|_| storage())?,
                    base_unit: InventoryBaseUnit::parse(&row.base_unit).map_err(|_| storage())?,
                    fractional_unit_allowed: row.fractional_unit_allowed != 0,
                    active: row.active != 0,
                    quantity_delta: Quantity::parse(&row.quantity_delta).map_err(|_| storage())?,
                })
            })
            .collect()
    }

    /// Devuelve el código de un artículo para los mensajes de faltante.
    pub(crate) async fn inventory_item_code(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> Result<CatalogCode, ApplicationError> {
        let code: Option<String> = sqlx::query_scalar(
            "SELECT code FROM inventory_item WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(inventory_item_id.as_str())
        .fetch_optional(&mut *self.connection())
        .await
        .map_err(|_| storage())?;
        CatalogCode::restore(&code.ok_or(ApplicationError::InventoryItemNotFound)?)
            .map_err(|_| storage())
    }
}

pub use pos_application::inventory::{
    AdjustStockRequest, InventoryApplication, InventoryPage, InventoryTransaction,
    InventoryUnitOfWork, MAX_INVENTORY_PAGE_SIZE, NewInventoryItemRequest, OpenStockRequest,
};

use super::queries::*;

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteInventoryApplication = InventoryApplication<SqliteUnitOfWork>;

impl InventoryUnitOfWork for SqliteUnitOfWork {
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

impl InventoryTransaction for SqliteTransaction {
    fn get_inventory_item(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<InventoryItem>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            self.get_inventory_item(&organization_id, &inventory_item_id)
                .await
        }
    }

    fn inventory_item_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let code = code.clone();
        async move {
            self.inventory_item_code_exists(&organization_id, &code)
                .await
        }
    }

    fn insert_inventory_item(
        &mut self,
        item: &InventoryItem,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let item = item.clone();
        let occurred_at = occurred_at.to_owned();
        async move { self.insert_inventory_item(&item, &occurred_at).await }
    }

    fn lock_inventory_unit(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.lock_inventory_unit(&organization_id, &inventory_item_id, &occurred_at)
                .await
        }
    }

    fn get_stock_balance(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<StockBalance>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            self.get_stock_balance(&organization_id, &branch_id, &inventory_item_id)
                .await
        }
    }

    fn insert_stock_balance(
        &mut self,
        balance: &StockBalance,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let balance = balance.clone();
        let occurred_at = occurred_at.to_owned();
        async move { self.insert_stock_balance(&balance, &occurred_at).await }
    }

    fn update_stock_balance_cas(
        &mut self,
        previous_revision: u64,
        balance: &StockBalance,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let balance = balance.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.update_stock_balance_cas(previous_revision, &balance, &occurred_at)
                .await
        }
    }

    fn ledger_counts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<(bool, bool), ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            self.ledger_counts(&organization_id, &branch_id, &inventory_item_id)
                .await
        }
    }

    fn insert_inventory_movement(
        &mut self,
        movement: &InventoryMovement,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let movement = movement.clone();
        async move { self.insert_inventory_movement(&movement).await }
    }

    fn list_inventory_movements(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<InventoryMovement>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            self.list_inventory_movements(
                &organization_id,
                &branch_id,
                &inventory_item_id,
                limit,
                offset,
            )
            .await
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
    fn list_inventory_items(
        &mut self,
        organization_id: &EntityId,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<InventoryItem>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        async move {
            self.list_inventory_items(&organization_id, limit, offset)
                .await
        }
    }
}

fn storage() -> ApplicationError {
    ApplicationError::Storage
}

impl SqliteTransaction {
    pub(crate) async fn list_inventory_items(
        &mut self,
        organization_id: &EntityId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InventoryItem>, ApplicationError> {
        let rows = sqlx::query_as::<_, InventoryItemRow>(LIST_ITEMS)
            .bind(organization_id.as_str())
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *self.connection())
            .await
            .map_err(|_| storage())?;
        rows.into_iter()
            .map(InventoryItemRow::into_domain)
            .collect::<Result<Vec<_>, _>>()
    }
}
