use super::*;
use crate::organization_branch::EntityId;

/// Saldo materializado de un artículo en una sucursal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockBalance {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    inventory_item_id: EntityId,
    quantity_on_hand: Quantity,
    revision: u64,
}

impl StockBalance {
    /// Saldo inicial en cero con revisión `0`.
    ///
    /// Una fila ausente se lee como cero; este constructor modela esa fila.
    pub fn empty(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        inventory_item_id: EntityId,
    ) -> Self {
        Self {
            id,
            organization_id,
            branch_id,
            inventory_item_id,
            quantity_on_hand: Quantity::zero(),
            revision: 0,
        }
    }

    /// Rehidrata un saldo persistido.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        inventory_item_id: EntityId,
        quantity_on_hand: Quantity,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            branch_id,
            inventory_item_id,
            quantity_on_hand,
            revision,
        }
    }

    /// Identidad de la fila de saldo.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Sucursal del saldo.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    /// Artículo del saldo.
    pub fn inventory_item_id(&self) -> &EntityId {
        &self.inventory_item_id
    }

    /// Cantidad vigente.
    pub fn quantity_on_hand(&self) -> Quantity {
        self.quantity_on_hand
    }

    /// Revisión optimista del saldo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Comprueba la pertenencia al scope indicado.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::OwnershipMismatch`].
    pub fn ensure_scope(
        &self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        inventory_item_id: &EntityId,
    ) -> Result<(), InventoryError> {
        if &self.organization_id != organization_id
            || &self.branch_id != branch_id
            || &self.inventory_item_id != inventory_item_id
        {
            return Err(InventoryError::OwnershipMismatch);
        }
        Ok(())
    }

    /// Aplica un delta bajo CAS y devuelve el saldo resultante.
    ///
    /// No aplica la regla de unidad ni la política de stock negativo: esas
    /// comprobaciones las realizan [`InventoryItem::ensure_quantity_fits_unit`] y
    /// [`ensure_sufficient`] en la misma decisión de negocio.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::ConcurrentModification`],
    /// [`InventoryError::QuantityOverflow`] o [`InventoryError::RevisionOverflow`].
    pub fn apply_delta(
        &mut self,
        expected_version: u64,
        delta: Quantity,
    ) -> Result<Quantity, InventoryError> {
        if expected_version != self.revision {
            return Err(InventoryError::ConcurrentModification);
        }
        let resulting = self.quantity_on_hand.checked_add(&delta)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(InventoryError::RevisionOverflow)?;
        self.quantity_on_hand = resulting;
        self.revision = revision;
        Ok(resulting)
    }
}
