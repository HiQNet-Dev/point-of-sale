use std::collections::BTreeSet;

use crate::catalog::{Decimal4, PriceDelta, UnitPrice};
use crate::organization_branch::EntityId;

use super::SaleError;

/// Grupo de modificadores asignado al producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierGroupAssignment {
    /// Grupo asignado.
    pub group_id: EntityId,
    /// Estado del grupo.
    pub active: bool,
    /// Mínimo de opciones exigidas.
    pub min_selections: u32,
    /// Máximo de opciones admitidas.
    pub max_selections: u32,
}

/// Opción seleccionada, ya resuelta contra el catálogo de la organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedModifierOption {
    /// Grupo al que pertenece la opción.
    pub group_id: EntityId,
    /// Opción seleccionada.
    pub option_id: EntityId,
    /// Estado de la opción.
    pub active: bool,
    /// Delta firmado de la opción.
    pub price_delta: PriceDelta,
    /// Orden de presentación dentro del grupo.
    pub display_order: u32,
}

/// Resultado de cotizar una configuración de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierPricing {
    /// Suma firmada de deltas.
    pub delta: PriceDelta,
    /// Precio unitario efectivo.
    pub effective_unit_price: UnitPrice,
    /// Opciones seleccionadas en orden canónico `(group_id, option_id)`.
    pub ordered: Vec<(EntityId, EntityId)>,
}

/// Cotiza la configuración de modificadores de un producto.
///
/// Valida pertenencia, actividad y límites por grupo, y produce el orden
/// canónico: grupos según su asignación al producto y opciones por
/// `display_order` y `option_id`.
///
/// # Errors
/// Devuelve [`SaleError::DuplicateModifierOption`],
/// [`SaleError::ModifierOptionNotAssigned`], [`SaleError::ModifierOptionInactive`],
/// [`SaleError::RequiredModifierGroupInactive`],
/// [`SaleError::ModifierGroupInactiveSelection`],
/// [`SaleError::ModifierSelectionOutOfBounds`],
/// [`SaleError::NegativeEffectivePrice`] o [`SaleError::AmountOutOfRange`].
pub fn resolve_modifier_pricing(
    base_price: UnitPrice,
    assignments: &[ModifierGroupAssignment],
    selected: &[SelectedModifierOption],
) -> Result<ModifierPricing, SaleError> {
    let mut seen = BTreeSet::new();
    for option in selected {
        if !seen.insert(&option.option_id) {
            return Err(SaleError::DuplicateModifierOption);
        }
        if !assignments
            .iter()
            .any(|assignment| assignment.group_id == option.group_id)
        {
            return Err(SaleError::ModifierOptionNotAssigned);
        }
        if !option.active {
            return Err(SaleError::ModifierOptionInactive);
        }
    }

    for assignment in assignments {
        let count = selected
            .iter()
            .filter(|option| option.group_id == assignment.group_id)
            .count();
        if !assignment.active {
            if assignment.min_selections > 0 {
                return Err(SaleError::RequiredModifierGroupInactive);
            }
            if count > 0 {
                return Err(SaleError::ModifierGroupInactiveSelection);
            }
        } else if count < assignment.min_selections as usize
            || count > assignment.max_selections as usize
        {
            return Err(SaleError::ModifierSelectionOutOfBounds);
        }
    }

    let mut delta_minor: i128 = 0;
    for option in selected {
        delta_minor = delta_minor
            .checked_add(option.price_delta.value().as_minor())
            .ok_or(SaleError::AmountOutOfRange)?;
    }
    let delta_amount =
        Decimal4::from_minor(delta_minor).map_err(|_| SaleError::AmountOutOfRange)?;
    let effective_minor = base_price
        .value()
        .as_minor()
        .checked_add(delta_minor)
        .ok_or(SaleError::AmountOutOfRange)?;
    if effective_minor < 0 {
        return Err(SaleError::NegativeEffectivePrice);
    }
    let effective_amount =
        Decimal4::from_minor(effective_minor).map_err(|_| SaleError::AmountOutOfRange)?;
    let effective_unit_price =
        UnitPrice::new(effective_amount).map_err(|_| SaleError::NegativeEffectivePrice)?;

    let mut ordered = Vec::new();
    for assignment in assignments {
        let mut options: Vec<&SelectedModifierOption> = selected
            .iter()
            .filter(|option| option.group_id == assignment.group_id)
            .collect();
        options.sort_by(|left, right| {
            left.display_order
                .cmp(&right.display_order)
                .then_with(|| left.option_id.cmp(&right.option_id))
        });
        for option in options {
            ordered.push((assignment.group_id.clone(), option.option_id.clone()));
        }
    }

    Ok(ModifierPricing {
        delta: PriceDelta::new(delta_amount),
        effective_unit_price,
        ordered,
    })
}
