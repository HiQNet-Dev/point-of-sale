//! Checkout candidato sobre SQLite real: pagos, consumo y cierre de venta en una
//! única unidad de trabajo.
//!
//! **No productivo.** Las tablas `payment` y `sale_item_inventory_component`
//! viven en `proposals/local-sales/0015` y se aplican solo en pruebas. No existe
//! ruta de aplicación que marque una venta `COMPLETED` sin pagos, sin resolución
//! de recetas y sin movimientos de ledger en la misma transacción.
//!
//! # Orden de la transacción
//!
//! 1. Autorización `sale.checkout` y scope derivado del principal.
//! 2. Lectura de la venta con CAS de revisión y exigencia de `OPEN`.
//! 3. Recalculo exacto del total desde las líneas activas.
//! 4. Validación de la composición de pagos antes de escribir nada.
//! 5. Inserción de pagos y snapshots de consumo por línea.
//! 6. Verificación de existencias según la política de la organización.
//! 7. Movimientos de ledger y saldos por CAS.
//! 8. Transición a `COMPLETED` con CAS y auditoría obligatoria.
//! 9. Commit único; cualquier fallo revierte todo el conjunto.

use pos_core::{
    audit::AuditOccurredAt,
    catalog::UnitPrice,
    identity::VerifiedPrincipal,
    inventory::{
        ConsumptionRequirement, InventoryError, InventoryMovement, NewMovement, Quantity,
        StockBalance,
    },
    organization_branch::{ApplicationError, EntityId, IdentityGenerator, Permission},
    recipe::{
        ResolvedConsumptionComponent, accumulate_line_totals, resolve_consumption,
        snapshot_line_consumption,
    },
    sales::{
        CheckoutInstruction, NewPayment, Payment, PaymentMethod, Sale, SaleItem,
        total_of_active_items, validate_checkout,
    },
};
use serde_json::json;

use pos_application::{
    audit::append_audit, authorization::require_sale_scope,
    sales_context::require_open_operational_shift,
};

use crate::{
    CasOutcome, SqliteTransaction, SqliteUnitOfWork,
    inventory_store::enforce_stock_policy,
    sales_store::{finish, sale_error, storage_error, transaction_error},
};

const INSERT_PAYMENT: &str = "INSERT INTO payment (id, organization_id, branch_id, \
    cash_register_id, cash_shift_id, sale_id, position, method, amount, tendered_amount, \
    change_amount, created_by_user_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
const INSERT_INVENTORY_COMPONENT: &str = "INSERT INTO sale_item_inventory_component \
    (id, organization_id, sale_item_id, inventory_item_id, base_unit, quantity_per_unit, \
    quantity_total, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

/// Pago solicitado por el transporte; los importes se validan en el dominio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckoutPaymentRequest {
    /// Medio de pago canónico.
    pub method: PaymentMethod,
    /// Importe aplicado como decimal exacto de escala 4.
    pub amount: String,
    /// Efectivo entregado; obligatorio para `CASH` y prohibido para `CARD`.
    pub tendered_amount: Option<String>,
}

/// Entrada completa de checkout de una venta abierta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckoutRequest {
    /// Caja del scope operativo.
    pub cash_register_id: EntityId,
    /// Turno OPEN de esa caja.
    pub cash_shift_id: EntityId,
    /// Venta a cobrar.
    pub sale_id: EntityId,
    /// Revisión leída antes de iniciar el checkout.
    pub expected_sale_version: u64,
    /// Pagos declarados por el operador.
    pub payments: Vec<CheckoutPaymentRequest>,
}

/// Resultado de un checkout confirmado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckoutOutcome {
    /// Venta ya `COMPLETED`.
    pub sale: Sale,
    /// Pagos persistidos en orden de posición.
    pub payments: Vec<Payment>,
    /// Consumo total por artículo aplicado al ledger.
    pub consumption: Vec<ResolvedConsumptionComponent>,
}

/// Casos de uso de checkout candidato. No se registra en el composition root
/// productivo mientras el DDL siga pendiente de aprobación.
#[derive(Clone)]
pub struct SqliteCheckoutApplication {
    unit_of_work: SqliteUnitOfWork,
}

impl SqliteCheckoutApplication {
    /// Ensambla checkout candidato sobre una UoW SQLite configurada.
    pub fn new(unit_of_work: SqliteUnitOfWork) -> Self {
        Self { unit_of_work }
    }

    /// Cobra una venta `OPEN` aplicando pagos, consumo de inventario y auditoría
    /// en la misma transacción.
    pub async fn checkout_sale<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: CheckoutRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CheckoutOutcome, ApplicationError> {
        let scope = require_sale_scope(
            principal,
            Permission::SaleCheckout,
            false,
            request.cash_register_id.clone(),
            request.cash_shift_id.clone(),
        )?;
        let instructions = parse_instructions(&request.payments)?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let mut sale = transaction
                .get_sale(&scope, &request.sale_id)
                .await
                .map_err(storage_error)?
                .ok_or(ApplicationError::SaleNotFound)?;
            // El estado manda sobre la revisión: un segundo checkout reporta
            // que la venta ya no está abierta en vez de un conflicto de versión.
            sale.require_open().map_err(sale_error)?;
            if sale.revision() != request.expected_sale_version {
                return Err(ApplicationError::ConcurrentModification);
            }
            require_open_operational_shift(&mut transaction, &scope).await?;

            let items = transaction
                .list_sale_items(&scope.organization_id, &request.sale_id)
                .await
                .map_err(storage_error)?;
            let active: Vec<&SaleItem> = items.iter().filter(|item| item.is_active()).collect();
            let total = total_of_active_items(&items).map_err(sale_error)?;
            if total != sale.total_amount() {
                // El total persistido y las líneas discrepan: no se cobra nada.
                return Err(ApplicationError::Storage);
            }
            validate_checkout(total, active.len(), &instructions).map_err(sale_error)?;

            let payments = write_payments(
                &mut transaction,
                identities,
                &scope,
                &sale,
                principal,
                &instructions,
                occurred_at,
            )
            .await?;

            let snapshots =
                write_line_snapshots(&mut transaction, identities, &scope, &active, occurred_at)
                    .await?;
            let consumption = accumulate_line_totals(&snapshots)
                .map_err(|_| ApplicationError::InvalidInventoryInput)?;

            apply_consumption(
                &mut transaction,
                identities,
                &scope,
                &sale,
                principal,
                &consumption,
                occurred_at,
            )
            .await?;

            let previous_revision = sale.revision();
            let total_snapshot = sale.total_amount();
            sale.complete(
                principal.user_id().clone(),
                transaction
                    .candidate_utc_seconds(occurred_at)
                    .await
                    .map_err(|_| ApplicationError::Storage)?,
            )
            .map_err(sale_error)?;
            if transaction
                .update_sale_cas(previous_revision, &sale, occurred_at.as_str())
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
                "sale.completed",
                "sale",
                sale.id().clone(),
                json!({
                    "paymentCount": payments.len(),
                    "consumedItems": consumption.len(),
                    "total": total_snapshot.to_string(),
                }),
                occurred_at,
            )
            .await?;
            Ok(CheckoutOutcome {
                sale,
                payments,
                consumption,
            })
        }
        .await;
        finish(transaction, operation).await
    }
}

fn parse_instructions(
    payments: &[CheckoutPaymentRequest],
) -> Result<Vec<CheckoutInstruction>, ApplicationError> {
    payments
        .iter()
        .map(|payment| {
            Ok(CheckoutInstruction {
                method: payment.method,
                amount: UnitPrice::parse(&payment.amount)
                    .map_err(|_| ApplicationError::InvalidSaleInput)?,
                tendered_amount: match payment.tendered_amount.as_deref() {
                    Some(value) => Some(
                        UnitPrice::parse(value).map_err(|_| ApplicationError::InvalidSaleInput)?,
                    ),
                    None => None,
                },
            })
        })
        .collect()
}

async fn write_payments<I: IdentityGenerator>(
    transaction: &mut SqliteTransaction,
    identities: &mut I,
    scope: &pos_core::sales::SaleScope,
    sale: &Sale,
    principal: &VerifiedPrincipal,
    instructions: &[CheckoutInstruction],
    occurred_at: &AuditOccurredAt,
) -> Result<Vec<Payment>, ApplicationError> {
    let mut payments = Vec::with_capacity(instructions.len());
    for (index, instruction) in instructions.iter().enumerate() {
        let position = u32::try_from(index + 1).map_err(|_| ApplicationError::InvalidSaleInput)?;
        let payment = Payment::create(NewPayment {
            id: identities.next_id()?,
            scope: scope.clone(),
            sale_id: sale.id().clone(),
            position,
            method: instruction.method,
            amount: instruction.amount,
            tendered_amount: instruction.tendered_amount,
            created_by_user_id: principal.user_id().clone(),
        })
        .map_err(sale_error)?;
        transaction
            .insert_payment(&payment, occurred_at.as_str())
            .await?;
        payments.push(payment);
    }
    Ok(payments)
}

async fn write_line_snapshots<I: IdentityGenerator>(
    transaction: &mut SqliteTransaction,
    identities: &mut I,
    scope: &pos_core::sales::SaleScope,
    active: &[&SaleItem],
    occurred_at: &AuditOccurredAt,
) -> Result<Vec<pos_core::recipe::SaleInventorySnapshot>, ApplicationError> {
    let mut snapshots = Vec::new();
    for item in active {
        let Some(recipe) = transaction
            .active_recipe(&scope.organization_id, item.product_id())
            .await?
        else {
            continue;
        };
        let effects = transaction
            .sale_item_modifier_effects(&scope.organization_id, item.id())
            .await?;
        let resolved = resolve_consumption(true, &recipe, &effects)
            .map_err(|_| ApplicationError::InvalidInventoryInput)?;
        if !resolved.tracked {
            continue;
        }
        let line = snapshot_line_consumption(&resolved, item.quantity())
            .map_err(|_| ApplicationError::InvalidInventoryInput)?;
        for snapshot in &line {
            transaction
                .insert_sale_item_inventory_component(
                    identities.next_id()?,
                    &scope.organization_id,
                    item.id(),
                    snapshot,
                    occurred_at.as_str(),
                )
                .await?;
        }
        snapshots.extend(line);
    }
    Ok(snapshots)
}

async fn apply_consumption<I: IdentityGenerator>(
    transaction: &mut SqliteTransaction,
    identities: &mut I,
    scope: &pos_core::sales::SaleScope,
    sale: &Sale,
    principal: &VerifiedPrincipal,
    consumption: &[ResolvedConsumptionComponent],
    occurred_at: &AuditOccurredAt,
) -> Result<(), ApplicationError> {
    if consumption.is_empty() {
        return Ok(());
    }
    let organization = transaction
        .get_organization(&scope.organization_id)
        .await
        .map_err(transaction_error)?
        .ok_or(ApplicationError::OrganizationNotFound)?;
    let mut requirements = Vec::with_capacity(consumption.len());
    let mut balances = Vec::with_capacity(consumption.len());
    for component in consumption {
        let code = transaction
            .inventory_item_code(&scope.organization_id, &component.inventory_item_id)
            .await?;
        requirements.push(ConsumptionRequirement {
            inventory_item_id: component.inventory_item_id.clone(),
            inventory_item_code: code,
            required_quantity: component.quantity,
        });
        let balance = transaction
            .get_stock_balance(
                &scope.organization_id,
                &scope.branch_id,
                &component.inventory_item_id,
            )
            .await?;
        balances.push((
            component.inventory_item_id.clone(),
            balance
                .as_ref()
                .map_or(Quantity::zero(), StockBalance::quantity_on_hand),
        ));
    }
    enforce_stock_policy(
        organization.negative_stock_policy(),
        &balances,
        &requirements,
    )?;

    for component in consumption {
        let mut balance = transaction
            .get_stock_balance(
                &scope.organization_id,
                &scope.branch_id,
                &component.inventory_item_id,
            )
            .await?
            .ok_or(ApplicationError::InvalidInventoryInput)?;
        let previous_revision = balance.revision();
        let delta = component
            .quantity
            .checked_neg()
            .map_err(|_| ApplicationError::InvalidInventoryInput)?;
        let resulting =
            balance
                .apply_delta(previous_revision, delta)
                .map_err(|error| match error {
                    InventoryError::ConcurrentModification => {
                        ApplicationError::ConcurrentModification
                    }
                    _ => ApplicationError::InvalidInventoryInput,
                })?;
        if transaction
            .update_stock_balance_cas(previous_revision, &balance, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            return Err(ApplicationError::ConcurrentModification);
        }
        let movement = InventoryMovement::sale_consumption(
            NewMovement {
                id: identities.next_id()?,
                organization_id: scope.organization_id.clone(),
                branch_id: scope.branch_id.clone(),
                inventory_item_id: component.inventory_item_id.clone(),
                actor_user_id: principal.user_id().clone(),
                occurred_at: transaction
                    .candidate_utc_seconds(occurred_at)
                    .await
                    .map_err(|_| ApplicationError::Storage)?,
            },
            delta,
            resulting,
            sale.id().clone(),
        )
        .map_err(|_| ApplicationError::InvalidInventoryInput)?;
        transaction.insert_inventory_movement(&movement).await?;
    }
    Ok(())
}

impl SqliteTransaction {
    async fn insert_payment(
        &mut self,
        payment: &Payment,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(INSERT_PAYMENT)
            .bind(payment.id().as_str())
            .bind(payment.scope().organization_id.as_str())
            .bind(payment.scope().branch_id.as_str())
            .bind(payment.scope().cash_register_id.as_str())
            .bind(payment.scope().cash_shift_id.as_str())
            .bind(payment.sale_id().as_str())
            .bind(i64::from(payment.position()))
            .bind(payment.method().as_str())
            .bind(payment.amount().to_string())
            .bind(payment.tendered_amount().map(|value| value.to_string()))
            .bind(payment.change_amount().to_string())
            .bind(payment.created_by_user_id().as_str())
            .bind(occurred_at)
            .execute(&mut *self.connection())
            .await
            .map_err(|error| map_checkout_write(&error))?;
        Ok(())
    }

    async fn insert_sale_item_inventory_component(
        &mut self,
        id: EntityId,
        organization_id: &EntityId,
        sale_item_id: &EntityId,
        snapshot: &pos_core::recipe::SaleInventorySnapshot,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(INSERT_INVENTORY_COMPONENT)
            .bind(id.as_str())
            .bind(organization_id.as_str())
            .bind(sale_item_id.as_str())
            .bind(snapshot.inventory_item_id.as_str())
            .bind(snapshot.base_unit.as_str())
            .bind(snapshot.quantity_per_unit.to_string())
            .bind(snapshot.quantity_total.to_string())
            .bind(occurred_at)
            .execute(&mut *self.connection())
            .await
            .map_err(|error| map_checkout_write(&error))?;
        Ok(())
    }
}

fn map_checkout_write(error: &sqlx::Error) -> ApplicationError {
    let Some(database) = error.as_database_error() else {
        return ApplicationError::Storage;
    };
    if database.is_foreign_key_violation() {
        ApplicationError::OwnershipMismatch
    } else if database.is_check_violation() || database.is_unique_violation() {
        ApplicationError::InvalidSaleInput
    } else {
        ApplicationError::Storage
    }
}
