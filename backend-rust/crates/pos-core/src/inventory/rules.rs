use super::*;
use crate::catalog::CatalogCode;
use crate::organization_branch::{EntityId, NegativeStockPolicy};

/// Requisito de consumo de una venta para un artículo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumptionRequirement {
    /// Artículo requerido.
    pub inventory_item_id: EntityId,
    /// Código del artículo al momento de la venta (para el mensaje de faltante).
    pub inventory_item_code: CatalogCode,
    /// Cantidad requerida, siempre positiva.
    pub required_quantity: Quantity,
}

/// Faltante detectado bajo política estricta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shortage {
    /// Código del artículo.
    pub inventory_item_code: CatalogCode,
    /// Cantidad requerida.
    pub required: Quantity,
    /// Cantidad disponible.
    pub available: Quantity,
}

/// Calcula los faltantes de un consumo bajo política estricta.
///
/// El orden de salida es el de `requirements`, igual que el servicio Java.
pub fn shortages(
    balances: &[(EntityId, Quantity)],
    requirements: &[ConsumptionRequirement],
) -> Vec<Shortage> {
    let mut found = Vec::new();
    for requirement in requirements {
        let available = balances
            .iter()
            .find(|(id, _)| id == &requirement.inventory_item_id)
            .map_or_else(Quantity::zero, |(_, quantity)| *quantity);
        if available < requirement.required_quantity {
            found.push(Shortage {
                inventory_item_code: requirement.inventory_item_code.clone(),
                required: requirement.required_quantity,
                available,
            });
        }
    }
    found
}

/// Regla de suficiencia del consumo de ventas.
///
/// Bajo `STRICT` el consumo es todo-o-nada; bajo `ALLOW_NEGATIVE` el saldo puede
/// quedar negativo. La política nunca se aplica a aperturas ni ajustes.
///
/// # Errors
/// Devuelve [`InventoryError::InsufficientInventory`] si falta existencias bajo
/// `STRICT`.
pub fn ensure_sufficient(
    policy: NegativeStockPolicy,
    balances: &[(EntityId, Quantity)],
    requirements: &[ConsumptionRequirement],
) -> Result<(), InventoryError> {
    if policy == NegativeStockPolicy::AllowNegative {
        return Ok(());
    }
    if shortages(balances, requirements).is_empty() {
        Ok(())
    } else {
        Err(InventoryError::InsufficientInventory)
    }
}

/// Regla de apertura: una sola apertura y antes de cualquier otro movimiento.
///
/// # Errors
/// Devuelve [`InventoryError::OpeningAlreadyExists`] o
/// [`InventoryError::HistoryAlreadyExists`], en el orden del servicio Java.
pub fn ensure_opening_allowed(
    has_opening: bool,
    has_any_movement: bool,
) -> Result<(), InventoryError> {
    if has_opening {
        return Err(InventoryError::OpeningAlreadyExists);
    }
    if has_any_movement {
        return Err(InventoryError::HistoryAlreadyExists);
    }
    Ok(())
}
