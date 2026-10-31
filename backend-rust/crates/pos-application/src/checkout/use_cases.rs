//! Caso de uso de checkout atómico.

use pos_core::{
    audit::AuditOccurredAt,
    catalog::UnitPrice,
    identity::VerifiedPrincipal,
    inventory::{
        ConsumptionRequirement, InventoryError, InventoryMovement, NewMovement, Quantity,
        StockBalance, ensure_sufficient,
    },
    organization_branch::{ApplicationError, IdentityGenerator, Permission},
    recipe::{
        ResolvedConsumptionComponent, accumulate_line_totals, resolve_consumption,
        snapshot_line_consumption,
    },
    sales::{
        CheckoutInstruction, NewPayment, Payment, Sale, SaleItem, total_of_active_items,
        validate_checkout,
    },
};
use serde_json::json;

use crate::{
    CasOutcome,
    audit::append_audit,
    authorization::require_sale_scope,
    checkout::{dto::*, ports::*},
    sales::{SalesTransaction, SalesUnitOfWork, sale_error},
    sales_context::require_open_operational_shift,
};

/// Casos de uso de checkout candidato. No se registra en el composition root
/// productivo mientras el DDL siga pendiente de aprobación.
#[derive(Clone)]
pub struct CheckoutApplication<U> {
    unit_of_work: U,
}

impl<U> CheckoutApplication<U>
where
    U: SalesUnitOfWork,
    U::Transaction: CheckoutTransaction + Send,
{
    /// Ensambla checkout candidato sobre una UoW SQLite configurada.
    pub fn new(unit_of_work: U) -> Self {
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
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let mut sale = transaction
                .get_sale(&scope, &request.sale_id)
                .await?
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
                .await?;
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
                .await?
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
        finish(&self.unit_of_work, transaction, operation).await
    }
}

async fn write_payments<I: IdentityGenerator>(
    transaction: &mut (impl CheckoutTransaction + Send),
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
    transaction: &mut (impl CheckoutTransaction + Send),
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
    transaction: &mut (impl CheckoutTransaction + Send),
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
        .await?
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
    ensure_sufficient(
        organization.negative_stock_policy(),
        &balances,
        &requirements,
    )
    .map_err(|_| ApplicationError::InsufficientInventory)?;

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
