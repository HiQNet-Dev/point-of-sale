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

use pos_application::{
    audit::append_audit,
    authorization::{require_operational_branch, require_organization_scope},
};
use pos_core::{
    audit::AuditOccurredAt,
    catalog::{CatalogCode, Description},
    identity::{UtcSeconds, VerifiedPrincipal}, /*  */
    inventory::{
        ConsumptionRequirement, InventoryBaseUnit, InventoryError, InventoryItem,
        InventoryMovement, InventoryMovementType, MovementReason, NewInventoryItem, NewMovement,
        Quantity, StockBalance, ensure_opening_allowed, ensure_sufficient,
    },
    organization_branch::{
        ApplicationError, DisplayName, EntityId, IdentityGenerator, NegativeStockPolicy,
        Permission, Status,
    },
    recipe::{ResolvedComponent, ResolvedEffect},
};
use serde_json::json;

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork, sales_store::finish};

/// Tamaño máximo de página para listados de inventario.
pub const MAX_INVENTORY_PAGE_SIZE: u32 = 100;

const SELECT_ITEM: &str = "SELECT id, organization_id, code, name, description, base_unit, \
    fractional_unit_allowed, unit_locked, status, version \
    FROM inventory_item WHERE organization_id = ? AND id = ?";
const LIST_ITEMS: &str = "SELECT id, organization_id, code, name, description, base_unit, \
    fractional_unit_allowed, unit_locked, status, version \
    FROM inventory_item WHERE organization_id = ? ORDER BY code, id LIMIT ? OFFSET ?";
const ITEM_CODE_EXISTS: &str =
    "SELECT EXISTS(SELECT 1 FROM inventory_item WHERE organization_id = ? AND code = ?)";
const INSERT_ITEM: &str = "INSERT INTO inventory_item (id, organization_id, code, name, description, \
    base_unit, fractional_unit_allowed, unit_locked, status, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?)";
const LOCK_UNIT: &str = "UPDATE inventory_item SET unit_locked = 1, updated_at = ? \
    WHERE organization_id = ? AND id = ? AND unit_locked = 0";

const SELECT_BALANCE: &str = "SELECT id, organization_id, branch_id, inventory_item_id, \
    quantity_on_hand, version FROM inventory_balance \
    WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ?";
const INSERT_BALANCE: &str = "INSERT INTO inventory_balance (id, organization_id, branch_id, \
    inventory_item_id, quantity_on_hand, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
const UPDATE_BALANCE_CAS: &str = "UPDATE inventory_balance SET quantity_on_hand = ?, version = ?, \
    updated_at = ? WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ? AND version = ?";
const BALANCE_EXISTS: &str = "SELECT EXISTS(SELECT 1 FROM inventory_balance \
    WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ?)";

const LEDGER_COUNTS: &str = "SELECT \
    EXISTS(SELECT 1 FROM inventory_ledger WHERE organization_id = ? AND branch_id = ? \
        AND inventory_item_id = ? AND movement_type = 'OPENING') AS has_opening, \
    EXISTS(SELECT 1 FROM inventory_ledger WHERE organization_id = ? AND branch_id = ? \
        AND inventory_item_id = ?) AS has_any";
const INSERT_MOVEMENT: &str = "INSERT INTO inventory_ledger (id, organization_id, branch_id, \
    inventory_item_id, movement_type, quantity_delta, resulting_quantity, reason, actor_user_id, \
    occurred_at, sale_id, purchase_receipt_line_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
const SELECT_MOVEMENTS: &str = "SELECT id, organization_id, branch_id, inventory_item_id, \
    movement_type, quantity_delta, resulting_quantity, reason, actor_user_id, occurred_at, \
    sale_id, purchase_receipt_line_id FROM inventory_ledger \
    WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ? \
    ORDER BY occurred_at DESC, id DESC LIMIT ? OFFSET ?";

const SELECT_ACTIVE_RECIPE: &str = "SELECT r.status AS recipe_status, \
    c.inventory_item_id AS inventory_item_id, c.quantity_per_product AS quantity_per_product, \
    i.base_unit AS base_unit, i.fractional_unit_allowed AS fractional_unit_allowed, \
    (i.status = 'ACTIVE') AS active \
    FROM product_recipe r \
    LEFT JOIN recipe_component c ON c.recipe_id = r.id AND c.organization_id = r.organization_id \
    LEFT JOIN inventory_item i ON i.id = c.inventory_item_id AND i.organization_id = c.organization_id \
    WHERE r.organization_id = ? AND r.product_id = ? \
    ORDER BY c.position, c.inventory_item_id";
const SELECT_ITEM_EFFECTS: &str = "SELECT e.inventory_item_id AS inventory_item_id, \
    i.base_unit AS base_unit, i.fractional_unit_allowed AS fractional_unit_allowed, \
    (i.status = 'ACTIVE') AS active, e.quantity_delta AS quantity_delta \
    FROM sale_item_modifier sim \
    JOIN modifier_inventory_effect e ON e.modifier_option_id = sim.modifier_option_id \
        AND e.organization_id = sim.organization_id \
    JOIN inventory_item i ON i.id = e.inventory_item_id AND i.organization_id = e.organization_id \
    WHERE sim.organization_id = ? AND sim.sale_item_id = ? \
    ORDER BY e.inventory_item_id";

fn storage() -> ApplicationError {
    ApplicationError::Storage
}

fn invalid() -> ApplicationError {
    ApplicationError::InvalidInventoryInput
}

fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}

fn revision_to_i64(value: u64) -> Result<i64, ApplicationError> {
    i64::try_from(value).map_err(|_| storage())
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

/// Entrada para crear un artículo de inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInventoryItemRequest {
    /// Código canónico; se normaliza igual que en Java.
    pub code: String,
    /// Nombre visible.
    pub name: String,
    /// Descripción opcional.
    pub description: Option<String>,
    /// Unidad base de todas las cantidades del artículo.
    pub base_unit: InventoryBaseUnit,
    /// Si el artículo admite cantidades fraccionarias.
    pub fractional_unit_allowed: bool,
}

/// Entrada de apertura de existencias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenStockRequest {
    /// Artículo de la organización del principal.
    pub inventory_item_id: EntityId,
    /// Cantidad declarada como existencias iniciales.
    pub quantity: String,
}

/// Entrada de ajuste manual con motivo obligatorio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjustStockRequest {
    /// Artículo de la organización del principal.
    pub inventory_item_id: EntityId,
    /// Delta firmado y distinto de cero.
    pub quantity_delta: String,
    /// Motivo normalizado del ajuste.
    pub reason: String,
}

/// Página determinista de artículos de inventario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryPage {
    /// Máximo de filas, entre uno y [`MAX_INVENTORY_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento en el orden por código e identidad.
    pub offset: u32,
}

/// Casos de uso candidatos de inventario. No forma parte del composition root
/// productivo mientras no exista DDL aprobado.
#[derive(Clone)]
pub struct SqliteInventoryApplication {
    unit_of_work: SqliteUnitOfWork,
}

impl SqliteInventoryApplication {
    /// Ensambla inventario candidato sobre una UoW SQLite configurada.
    pub fn new(unit_of_work: SqliteUnitOfWork) -> Self {
        Self { unit_of_work }
    }

    /// Crea un artículo activo; el código es único por organización.
    pub async fn create_item<I: IdentityGenerator>(
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
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
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
        finish(transaction, operation).await
    }

    /// Lee un artículo dentro de la organización del principal.
    pub async fn get_item(
        &self,
        principal: &VerifiedPrincipal,
        inventory_item_id: &EntityId,
    ) -> Result<InventoryItem, ApplicationError> {
        let organization_id = require_organization_scope(principal, Permission::InventoryItemRead)?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            transaction
                .get_inventory_item(&organization_id, inventory_item_id)
                .await?
                .ok_or(ApplicationError::InventoryItemNotFound)
        }
        .await;
        finish(transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let rows = sqlx::query_as::<_, InventoryItemRow>(LIST_ITEMS)
                .bind(organization_id.as_str())
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *transaction.connection())
                .await
                .map_err(|_| storage())?;
            rows.into_iter()
                .map(InventoryItemRow::into_domain)
                .collect::<Result<Vec<_>, _>>()
        }
        .await;
        finish(transaction, operation).await
    }

    /// Registra la apertura única de existencias de un artículo en la sucursal
    /// del principal verificado.
    pub async fn open_stock<I: IdentityGenerator>(
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
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
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
        finish(transaction, operation).await
    }

    /// Aplica un ajuste manual sobre un saldo previamente aperturado.
    pub async fn adjust_stock<I: IdentityGenerator>(
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
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
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
        finish(transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            transaction
                .get_stock_balance(&organization_id, &branch_id, inventory_item_id)
                .await
        }
        .await;
        finish(transaction, operation).await
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
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
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
        finish(transaction, operation).await
    }
}

/// Comprueba suficiencia de stock con la política de la organización.
///
/// Compartido con el checkout para no duplicar la regla de negocio.
pub(crate) fn enforce_stock_policy(
    policy: NegativeStockPolicy,
    balances: &[(EntityId, Quantity)],
    requirements: &[ConsumptionRequirement],
) -> Result<(), ApplicationError> {
    ensure_sufficient(policy, balances, requirements).map_err(inventory_error)
}
