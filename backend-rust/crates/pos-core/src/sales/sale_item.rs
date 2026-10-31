use crate::catalog::{Decimal4, PriceDelta, Sku, UnitPrice};
use crate::identity::UtcSeconds;
use crate::organization_branch::{DisplayName, EntityId};

use super::state::SaleItemStatus;
use super::{MAX_ITEM_QUANTITY, SaleError};

/// Datos de creación de una línea.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSaleItem {
    /// Identidad de la línea.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Venta a la que pertenece.
    pub sale_id: EntityId,
    /// Producto vendido.
    pub product_id: EntityId,
    /// SKU congelado.
    pub product_sku_snapshot: Sku,
    /// Nombre congelado.
    pub product_name_snapshot: DisplayName,
    /// Precio base de la organización.
    pub base_unit_price: UnitPrice,
    /// Suma firmada de modificadores.
    pub modifier_unit_delta: PriceDelta,
    /// Precio unitario efectivo, no negativo.
    pub effective_unit_price: UnitPrice,
    /// Cantidad de unidades.
    pub quantity: u32,
    /// Posición dentro de la venta.
    pub position: u32,
}

/// Línea de venta con snapshots inmutables del catálogo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaleItem {
    id: EntityId,
    organization_id: EntityId,
    sale_id: EntityId,
    product_id: EntityId,
    product_sku_snapshot: Sku,
    product_name_snapshot: DisplayName,
    base_unit_price: UnitPrice,
    modifier_unit_delta: PriceDelta,
    effective_unit_price: UnitPrice,
    quantity: u32,
    line_total: UnitPrice,
    status: SaleItemStatus,
    position: u32,
    removed_by_user_id: Option<EntityId>,
    removed_at: Option<UtcSeconds>,
}

impl SaleItem {
    /// Crea una línea activa calculando su total exacto.
    ///
    /// # Errors
    /// Devuelve [`SaleError::InvalidQuantity`] o [`SaleError::AmountOutOfRange`].
    pub fn create(new: NewSaleItem) -> Result<Self, SaleError> {
        validate_quantity(new.quantity)?;
        let line_total = line_total(new.effective_unit_price, new.quantity)?;
        Ok(Self {
            id: new.id,
            organization_id: new.organization_id,
            sale_id: new.sale_id,
            product_id: new.product_id,
            product_sku_snapshot: new.product_sku_snapshot,
            product_name_snapshot: new.product_name_snapshot,
            base_unit_price: new.base_unit_price,
            modifier_unit_delta: new.modifier_unit_delta,
            effective_unit_price: new.effective_unit_price,
            quantity: new.quantity,
            line_total,
            status: SaleItemStatus::Active,
            position: new.position,
            removed_by_user_id: None,
            removed_at: None,
        })
    }

    /// Rehidrata una línea persistida y verifica tanto el total como el estado histórico.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        sale_id: EntityId,
        product_id: EntityId,
        product_sku_snapshot: Sku,
        product_name_snapshot: DisplayName,
        base_unit_price: UnitPrice,
        modifier_unit_delta: PriceDelta,
        effective_unit_price: UnitPrice,
        quantity: u32,
        persisted_line_total: UnitPrice,
        status: SaleItemStatus,
        position: u32,
        removed_by_user_id: Option<EntityId>,
        removed_at: Option<UtcSeconds>,
    ) -> Result<Self, SaleError> {
        validate_quantity(quantity)?;
        if persisted_line_total != line_total(effective_unit_price, quantity)?
            || matches!(status, SaleItemStatus::Active)
                && (removed_by_user_id.is_some() || removed_at.is_some())
            || matches!(status, SaleItemStatus::Removed)
                && (removed_by_user_id.is_none() || removed_at.is_none())
        {
            return Err(SaleError::InvalidPersistedValue);
        }
        Ok(Self {
            id,
            organization_id,
            sale_id,
            product_id,
            product_sku_snapshot,
            product_name_snapshot,
            base_unit_price,
            modifier_unit_delta,
            effective_unit_price,
            quantity,
            line_total: persisted_line_total,
            status,
            position,
            removed_by_user_id,
            removed_at,
        })
    }

    /// Identidad de la línea.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Venta a la que pertenece.
    pub fn sale_id(&self) -> &EntityId {
        &self.sale_id
    }
    /// Producto vendido.
    pub fn product_id(&self) -> &EntityId {
        &self.product_id
    }
    /// SKU congelado.
    pub fn product_sku_snapshot(&self) -> &Sku {
        &self.product_sku_snapshot
    }
    /// Nombre congelado.
    pub fn product_name_snapshot(&self) -> &DisplayName {
        &self.product_name_snapshot
    }
    /// Precio base.
    pub fn base_unit_price(&self) -> UnitPrice {
        self.base_unit_price
    }
    /// Delta de modificadores.
    pub fn modifier_unit_delta(&self) -> PriceDelta {
        self.modifier_unit_delta
    }
    /// Precio unitario efectivo.
    pub fn effective_unit_price(&self) -> UnitPrice {
        self.effective_unit_price
    }
    /// Cantidad.
    pub fn quantity(&self) -> u32 {
        self.quantity
    }
    /// Total de la línea.
    pub fn line_total(&self) -> UnitPrice {
        self.line_total
    }
    /// Estado de la línea.
    pub fn status(&self) -> SaleItemStatus {
        self.status
    }
    /// Posición dentro de la venta.
    pub fn position(&self) -> u32 {
        self.position
    }
    /// Actor que retiró la línea, si aplica.
    pub fn removed_by_user_id(&self) -> Option<&EntityId> {
        self.removed_by_user_id.as_ref()
    }
    /// Instante de retirada, si aplica.
    pub fn removed_at(&self) -> Option<UtcSeconds> {
        self.removed_at
    }

    /// Indica si la línea cuenta para el total.
    pub fn is_active(&self) -> bool {
        self.status == SaleItemStatus::Active
    }

    /// Exige que la línea siga activa.
    ///
    /// # Errors
    /// Devuelve [`SaleError::ItemAlreadyRemoved`].
    pub fn require_active(&self) -> Result<(), SaleError> {
        if !self.is_active() {
            return Err(SaleError::ItemAlreadyRemoved);
        }
        Ok(())
    }

    /// Reemplaza la cotización de una línea activa.
    ///
    /// # Errors
    /// Devuelve [`SaleError::ItemAlreadyRemoved`],
    /// [`SaleError::InvalidQuantity`] o [`SaleError::AmountOutOfRange`].
    pub fn update_quote(
        &mut self,
        base_unit_price: UnitPrice,
        modifier_unit_delta: PriceDelta,
        effective_unit_price: UnitPrice,
        quantity: u32,
    ) -> Result<(), SaleError> {
        self.require_active()?;
        validate_quantity(quantity)?;
        let total = line_total(effective_unit_price, quantity)?;
        self.base_unit_price = base_unit_price;
        self.modifier_unit_delta = modifier_unit_delta;
        self.effective_unit_price = effective_unit_price;
        self.quantity = quantity;
        self.line_total = total;
        Ok(())
    }

    /// Retira la línea conservando su historial.
    ///
    /// # Errors
    /// Devuelve [`SaleError::ItemAlreadyRemoved`].
    pub fn remove(&mut self, actor: EntityId, at: UtcSeconds) -> Result<(), SaleError> {
        self.require_active()?;
        self.status = SaleItemStatus::Removed;
        self.removed_by_user_id = Some(actor);
        self.removed_at = Some(at);
        Ok(())
    }
}

fn validate_quantity(quantity: u32) -> Result<(), SaleError> {
    if quantity == 0 || quantity > MAX_ITEM_QUANTITY {
        return Err(SaleError::InvalidQuantity);
    }
    Ok(())
}

fn line_total(effective: UnitPrice, quantity: u32) -> Result<UnitPrice, SaleError> {
    let minor = effective
        .value()
        .as_minor()
        .checked_mul(i128::from(quantity))
        .ok_or(SaleError::AmountOutOfRange)?;
    let amount = Decimal4::from_minor(minor).map_err(|_| SaleError::AmountOutOfRange)?;
    UnitPrice::new(amount).map_err(|_| SaleError::AmountOutOfRange)
}
