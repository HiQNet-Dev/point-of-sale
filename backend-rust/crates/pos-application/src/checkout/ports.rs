//! Puertos de aplicación para checkout.
//!
//! Extiende el puerto de ventas con las operaciones de pago e inventario que
//! deben compartir la **misma** transacción física.

use std::future::Future;

use pos_core::{
    catalog::CatalogCode,
    inventory::{InventoryMovement, StockBalance},
    organization_branch::{ApplicationError, EntityId, Organization},
    recipe::{ResolvedComponent, ResolvedEffect, SaleInventorySnapshot},
    sales::Payment,
};

use crate::{CasOutcome, sales::SalesTransaction};

/// Operaciones transaccionales del checkout atómico.
pub trait CheckoutTransaction: SalesTransaction {
    /// Inserta un pago inmutable.
    fn insert_payment(
        &mut self,
        payment: &Payment,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Congela el consumo de una línea.
    fn insert_sale_item_inventory_component(
        &mut self,
        id: EntityId,
        organization_id: &EntityId,
        sale_item_id: &EntityId,
        snapshot: &SaleInventorySnapshot,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Receta vigente de un producto, si está trackeado.
    fn active_recipe(
        &mut self,
        organization_id: &EntityId,
        product_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Vec<ResolvedComponent>>, ApplicationError>> + Send;
    /// Efectos de inventario de las opciones de una línea.
    fn sale_item_modifier_effects(
        &mut self,
        organization_id: &EntityId,
        sale_item_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<ResolvedEffect>, ApplicationError>> + Send;
    /// Organización para leer la política de stock negativo.
    fn get_organization(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Organization>, ApplicationError>> + Send;
    /// Código de artículo para mensajes de faltante.
    fn inventory_item_code(
        &mut self,
        organization_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<CatalogCode, ApplicationError>> + Send;
    /// Saldo de un artículo en la sucursal.
    fn get_stock_balance(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> impl Future<Output = Result<Option<StockBalance>, ApplicationError>> + Send;
    /// CAS del saldo.
    fn update_stock_balance_cas(
        &mut self,
        previous_revision: u64,
        balance: &StockBalance,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Movimiento append-only del ledger.
    fn insert_inventory_movement(
        &mut self,
        movement: &InventoryMovement,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}
