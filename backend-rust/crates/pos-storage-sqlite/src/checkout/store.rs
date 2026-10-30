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

use std::future::Future;

use pos_core::{
    organization_branch::{ApplicationError, EntityId},
    recipe::{ResolvedComponent, ResolvedEffect, SaleInventorySnapshot},
    sales::Payment,
};

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork};

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

pub use pos_application::checkout::{
    CheckoutApplication, CheckoutOutcome, CheckoutPaymentRequest, CheckoutRequest,
    CheckoutTransaction,
};

use super::queries::*;

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteCheckoutApplication = CheckoutApplication<SqliteUnitOfWork>;

impl CheckoutTransaction for SqliteTransaction {
    fn insert_payment(
        &mut self,
        payment: &Payment,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let payment = payment.clone();
        let occurred_at = occurred_at.to_owned();
        async move { self.insert_payment(&payment, &occurred_at).await }
    }

    fn insert_sale_item_inventory_component(
        &mut self,
        id: EntityId,
        organization_id: &EntityId,
        sale_item_id: &EntityId,
        snapshot: &SaleInventorySnapshot,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let sale_item_id = sale_item_id.clone();
        let snapshot = snapshot.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.insert_sale_item_inventory_component(
                id,
                &organization_id,
                &sale_item_id,
                &snapshot,
                &occurred_at,
            )
            .await
        }
    }

    fn active_recipe(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Vec<ResolvedComponent>>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let product_id = product_id.clone();
        async move { self.active_recipe(&organization_id, &product_id).await }
    }

    fn sale_item_modifier_effects(
        &mut self,
        organization_id: &EntityId,
        sale_item_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<ResolvedEffect>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let sale_item_id = sale_item_id.clone();
        async move {
            self.sale_item_modifier_effects(&organization_id, &sale_item_id)
                .await
        }
    }

    fn get_organization(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<
        Output = Result<Option<pos_core::organization_branch::Organization>, ApplicationError>,
    > + Send {
        let organization_id = organization_id.clone();
        async move {
            self.get_organization(&organization_id)
                .await
                .map_err(|_| ApplicationError::Storage)
        }
    }

    fn inventory_item_code(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<pos_core::catalog::CatalogCode, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            self.inventory_item_code(&organization_id, &inventory_item_id)
                .await
        }
    }

    fn get_stock_balance(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<pos_core::inventory::StockBalance>, ApplicationError>> + Send
    {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let inventory_item_id = inventory_item_id.clone();
        async move {
            self.get_stock_balance(&organization_id, &branch_id, &inventory_item_id)
                .await
        }
    }

    fn update_stock_balance_cas(
        &mut self,
        previous_revision: u64,
        balance: &pos_core::inventory::StockBalance,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let balance = balance.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.update_stock_balance_cas(previous_revision, &balance, &occurred_at)
                .await
        }
    }

    fn insert_inventory_movement(
        &mut self,
        movement: &pos_core::inventory::InventoryMovement,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let movement = movement.clone();
        async move { self.insert_inventory_movement(&movement).await }
    }
}
