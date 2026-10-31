//! Dominio puro de recetas de producto y efectos de modificadores.
//!
//! Reproduce `inventory/recipe` y `inventory/modifier` de Java, además de la
//! resolución de consumo de `InventoryConsumptionResolver`, sin JPA, Spring,
//! SQLx ni transporte.
//!
//! # Modelo
//!
//! - Una **receta** pertenece a un producto del catálogo y declara cuánto
//!   inventario consume **una unidad vendida**.
//! - Una receta `ACTIVE` exige al menos un componente; una `INACTIVE` puede
//!   quedar vacía y significa "no trackeado".
//! - Los **efectos de modificador** suman o restan cantidades cuando se
//!   selecciona una opción.
//! - La resolución combina receta y efectos en una cantidad por artículo, con
//!   aritmética exacta de escala 6 y sin punto flotante.
//!
//! # Invariantes
//!
//! - Toda cantidad de componente es estrictamente positiva; todo efecto tiene
//!   delta distinto de cero.
//! - No se repite un artículo dentro de una receta ni dentro de los efectos de
//!   una opción.
//! - Un componente nuevo no puede referenciar un artículo inactivo; un
//!   componente preexistente idéntico puede conservarse mientras la receta se
//!   guarda como `INACTIVE`.
//! - La cantidad resuelta no puede ser negativa y respeta la regla de unidad:
//!   un artículo `UNIT` sin fracciones no admite totales fraccionarios.
//! - El orden de los componentes no tiene significado operativo: la resolución
//!   devuelve los artículos ordenados por identificador.

use std::collections::BTreeMap;
use std::fmt;

use crate::{
    inventory::{InventoryBaseUnit, Quantity},
    organization_branch::EntityId,
};

/// Estado de una receta de inventario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeStatus {
    /// La receta se aplica al consumo de ventas.
    Active,
    /// La receta se conserva pero no se aplica.
    Inactive,
}

impl RecipeStatus {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Inactive => "INACTIVE",
        }
    }

    /// Convierte un valor persistido sin aceptar estados desconocidos.
    ///
    /// # Errors
    /// Devuelve [`RecipeError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, RecipeError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "INACTIVE" => Ok(Self::Inactive),
            _ => Err(RecipeError::InvalidPersistedValue),
        }
    }
}

/// Fallos de recetas, efectos y resolución de consumo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeError {
    /// Una cantidad de componente debe ser estrictamente positiva.
    QuantityMustBePositive,
    /// La cantidad excede el rango de `NUMERIC(19,6)`.
    QuantityOutOfRange,
    /// El mismo artículo aparece dos veces.
    DuplicateComponent,
    /// Una receta activa requiere al menos un componente.
    ActiveRecipeRequiresComponents,
    /// Un componente nuevo referencia un artículo inactivo.
    InactiveItemNotAllowed,
    /// El artículo no admite cantidades fraccionarias.
    FractionalQuantityNotAllowed,
    /// La cantidad resuelta resulta negativa.
    NegativeResolvedQuantity,
    /// El delta de un efecto no puede ser cero.
    EffectDeltaMustNotBeZero,
    /// La cantidad de venta debe ser mayor que cero.
    InvalidSaleQuantity,
    /// La entidad pertenece a otra organización.
    OwnershipMismatch,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for RecipeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::QuantityMustBePositive => "la cantidad debe ser positiva",
            Self::QuantityOutOfRange => "la cantidad excede el rango representable",
            Self::DuplicateComponent => "el artículo aparece dos veces",
            Self::ActiveRecipeRequiresComponents => "una receta activa requiere componentes",
            Self::InactiveItemNotAllowed => "el artículo está inactivo",
            Self::FractionalQuantityNotAllowed => "el artículo no admite cantidades fraccionarias",
            Self::NegativeResolvedQuantity => "la cantidad resuelta es negativa",
            Self::EffectDeltaMustNotBeZero => "el delta del efecto no puede ser cero",
            Self::InvalidSaleQuantity => "cantidad de venta no válida",
            Self::OwnershipMismatch => "la entidad pertenece a otra organización",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for RecipeError {}

/// Componente de receta ya resuelto contra el catálogo de inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedComponent {
    /// Artículo consumido.
    pub inventory_item_id: EntityId,
    /// Unidad base vigente del artículo.
    pub base_unit: InventoryBaseUnit,
    /// Si el artículo admite fracciones.
    pub fractional_unit_allowed: bool,
    /// Estado vigente del artículo.
    pub active: bool,
    /// Cantidad consumida por unidad vendida.
    pub quantity_per_product: Quantity,
}

/// Efecto de una opción de modificador sobre el inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedEffect {
    /// Artículo afectado.
    pub inventory_item_id: EntityId,
    /// Unidad base vigente del artículo.
    pub base_unit: InventoryBaseUnit,
    /// Si el artículo admite fracciones.
    pub fractional_unit_allowed: bool,
    /// Estado vigente del artículo.
    pub active: bool,
    /// Delta firmado aplicado por la opción.
    pub quantity_delta: Quantity,
}

/// Resultado de planificar la sustitución de una receta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeChange {
    /// La receta solicitada es idéntica a la vigente: sin mutación ni auditoría.
    Unchanged,
    /// Receta nueva con estado y componentes canónicos.
    Replace {
        /// Estado solicitado.
        status: RecipeStatus,
        /// Componentes en el orden recibido.
        components: Vec<ResolvedComponent>,
    },
}

/// Resultado de planificar la sustitución de efectos de una opción.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectChange {
    /// Los efectos solicitados son idénticos a los vigentes.
    Unchanged,
    /// Efectos nuevos.
    Replace(Vec<ResolvedEffect>),
}

/// Planifica la sustitución de una receta.
///
/// `current` y `requested` deben llegar en un orden estable (por identificador
/// de artículo), igual que la lectura ordenada del repositorio Java.
///
/// # Errors
/// Devuelve [`RecipeError::DuplicateComponent`],
/// [`RecipeError::QuantityMustBePositive`],
/// [`RecipeError::FractionalQuantityNotAllowed`],
/// [`RecipeError::InactiveItemNotAllowed`] o
/// [`RecipeError::ActiveRecipeRequiresComponents`].
pub fn plan_recipe_replacement(
    current_status: Option<RecipeStatus>,
    current: &[ResolvedComponent],
    requested_status: RecipeStatus,
    requested: &[ResolvedComponent],
) -> Result<RecipeChange, RecipeError> {
    let mut seen = std::collections::BTreeSet::new();
    for component in requested {
        if !seen.insert(&component.inventory_item_id) {
            return Err(RecipeError::DuplicateComponent);
        }
        if component.quantity_per_product.is_negative() || component.quantity_per_product.is_zero()
        {
            return Err(RecipeError::QuantityMustBePositive);
        }
        ensure_unit_allows(
            component.base_unit,
            component.fractional_unit_allowed,
            component.quantity_per_product,
        )?;
        if !component.active {
            let retained_unchanged = requested_status == RecipeStatus::Inactive
                && current.iter().any(|prior| {
                    prior.inventory_item_id == component.inventory_item_id
                        && prior.quantity_per_product == component.quantity_per_product
                });
            if !retained_unchanged {
                return Err(RecipeError::InactiveItemNotAllowed);
            }
        }
    }
    if requested_status == RecipeStatus::Active && requested.is_empty() {
        return Err(RecipeError::ActiveRecipeRequiresComponents);
    }
    if current_status == Some(requested_status) && same_components(current, requested) {
        return Ok(RecipeChange::Unchanged);
    }
    Ok(RecipeChange::Replace {
        status: requested_status,
        components: requested.to_vec(),
    })
}

/// Planifica la sustitución de efectos de una opción de modificador.
///
/// # Errors
/// Devuelve [`RecipeError::DuplicateComponent`],
/// [`RecipeError::EffectDeltaMustNotBeZero`],
/// [`RecipeError::QuantityOutOfRange`] o [`RecipeError::InactiveItemNotAllowed`].
pub fn plan_effect_replacement(
    current: &[ResolvedEffect],
    requested: &[ResolvedEffect],
) -> Result<EffectChange, RecipeError> {
    let mut seen = std::collections::BTreeSet::new();
    for effect in requested {
        if !seen.insert(&effect.inventory_item_id) {
            return Err(RecipeError::DuplicateComponent);
        }
        if effect.quantity_delta.is_zero() {
            return Err(RecipeError::EffectDeltaMustNotBeZero);
        }
        if !effect.active {
            return Err(RecipeError::InactiveItemNotAllowed);
        }
    }
    if current == requested {
        return Ok(EffectChange::Unchanged);
    }
    Ok(EffectChange::Replace(requested.to_vec()))
}

fn same_components(current: &[ResolvedComponent], requested: &[ResolvedComponent]) -> bool {
    current.len() == requested.len()
        && current.iter().zip(requested).all(|(left, right)| {
            left.inventory_item_id == right.inventory_item_id
                && left.quantity_per_product == right.quantity_per_product
        })
}

/// Componente del consumo resuelto de una venta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConsumptionComponent {
    /// Artículo consumido.
    pub inventory_item_id: EntityId,
    /// Unidad base vigente del artículo.
    pub base_unit: InventoryBaseUnit,
    /// Cantidad total por unidad vendida.
    pub quantity: Quantity,
}

/// Consumo resuelto de una venta para una unidad del producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConsumption {
    /// `false` cuando el producto no está trackeado por inventario.
    pub tracked: bool,
    /// Componentes ordenados por identificador de artículo.
    pub components: Vec<ResolvedConsumptionComponent>,
}

impl ResolvedConsumption {
    /// Consumo de un producto sin receta activa.
    pub fn untracked() -> Self {
        Self {
            tracked: false,
            components: Vec::new(),
        }
    }
}

/// Resuelve el consumo de una unidad vendida combinando receta y efectos.
///
/// # Errors
/// Devuelve [`RecipeError::InactiveItemNotAllowed`],
/// [`RecipeError::QuantityOutOfRange`],
/// [`RecipeError::NegativeResolvedQuantity`] o
/// [`RecipeError::FractionalQuantityNotAllowed`].
pub fn resolve_consumption(
    recipe_active: bool,
    components: &[ResolvedComponent],
    effects: &[ResolvedEffect],
) -> Result<ResolvedConsumption, RecipeError> {
    if !recipe_active {
        return Ok(ResolvedConsumption::untracked());
    }
    let mut totals: BTreeMap<EntityId, (InventoryBaseUnit, bool, Quantity)> = BTreeMap::new();
    for component in components {
        ensure_active(component.active)?;
        add(
            &mut totals,
            &component.inventory_item_id,
            component.base_unit,
            component.fractional_unit_allowed,
            component.quantity_per_product,
        )?;
    }
    for effect in effects {
        ensure_active(effect.active)?;
        add(
            &mut totals,
            &effect.inventory_item_id,
            effect.base_unit,
            effect.fractional_unit_allowed,
            effect.quantity_delta,
        )?;
    }

    let mut resolved = Vec::with_capacity(totals.len());
    for (inventory_item_id, (base_unit, fractional_unit_allowed, quantity)) in totals {
        if quantity.is_negative() {
            return Err(RecipeError::NegativeResolvedQuantity);
        }
        if quantity.is_zero() {
            continue;
        }
        ensure_unit_allows(base_unit, fractional_unit_allowed, quantity)?;
        resolved.push(ResolvedConsumptionComponent {
            inventory_item_id,
            base_unit,
            quantity,
        });
    }
    Ok(ResolvedConsumption {
        tracked: true,
        components: resolved,
    })
}

fn ensure_active(active: bool) -> Result<(), RecipeError> {
    if active {
        Ok(())
    } else {
        Err(RecipeError::InactiveItemNotAllowed)
    }
}

fn add(
    totals: &mut BTreeMap<EntityId, (InventoryBaseUnit, bool, Quantity)>,
    inventory_item_id: &EntityId,
    base_unit: InventoryBaseUnit,
    fractional_unit_allowed: bool,
    quantity: Quantity,
) -> Result<(), RecipeError> {
    match totals.get_mut(inventory_item_id) {
        Some((_, _, current)) => {
            *current = current
                .checked_add(&quantity)
                .map_err(|_| RecipeError::QuantityOutOfRange)?;
        }
        None => {
            totals.insert(
                inventory_item_id.clone(),
                (base_unit, fractional_unit_allowed, quantity),
            );
        }
    }
    Ok(())
}

fn ensure_unit_allows(
    base_unit: InventoryBaseUnit,
    fractional_unit_allowed: bool,
    quantity: Quantity,
) -> Result<(), RecipeError> {
    if base_unit == InventoryBaseUnit::Unit && !fractional_unit_allowed && quantity.is_fractional()
    {
        return Err(RecipeError::FractionalQuantityNotAllowed);
    }
    Ok(())
}

/// Snapshot de consumo congelado en una línea de venta.
///
/// Corresponde a `sale_item_inventory_components` (V19): guarda la cantidad por
/// unidad y la cantidad total vendida, de modo que el historial no dependa de la
/// receta vigente.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaleInventorySnapshot {
    /// Artículo consumido.
    pub inventory_item_id: EntityId,
    /// Unidad base al momento de la venta.
    pub base_unit: InventoryBaseUnit,
    /// Cantidad por unidad vendida.
    pub quantity_per_unit: Quantity,
    /// Cantidad total de la línea.
    pub quantity_total: Quantity,
}

/// Convierte el consumo de una unidad en snapshots para una línea de venta.
///
/// # Errors
/// Devuelve [`RecipeError::InvalidSaleQuantity`] si la cantidad es cero o
/// [`RecipeError::QuantityOutOfRange`] si el producto excede el rango.
pub fn snapshot_line_consumption(
    resolved: &ResolvedConsumption,
    sale_quantity: u32,
) -> Result<Vec<SaleInventorySnapshot>, RecipeError> {
    if sale_quantity == 0 {
        return Err(RecipeError::InvalidSaleQuantity);
    }
    let mut snapshots = Vec::with_capacity(resolved.components.len());
    for component in &resolved.components {
        let total_minor = component
            .quantity
            .as_minor()
            .checked_mul(i128::from(sale_quantity))
            .ok_or(RecipeError::QuantityOutOfRange)?;
        let quantity_total =
            Quantity::from_minor(total_minor).map_err(|_| RecipeError::QuantityOutOfRange)?;
        snapshots.push(SaleInventorySnapshot {
            inventory_item_id: component.inventory_item_id.clone(),
            base_unit: component.base_unit,
            quantity_per_unit: component.quantity,
            quantity_total,
        });
    }
    Ok(snapshots)
}

/// Acumula los totales por artículo de varias líneas de una misma venta.
///
/// # Errors
/// Devuelve [`RecipeError::QuantityOutOfRange`] si la suma excede el rango.
pub fn accumulate_line_totals(
    snapshots: &[SaleInventorySnapshot],
) -> Result<Vec<ResolvedConsumptionComponent>, RecipeError> {
    let mut totals: BTreeMap<EntityId, (InventoryBaseUnit, Quantity)> = BTreeMap::new();
    for snapshot in snapshots {
        match totals.get_mut(&snapshot.inventory_item_id) {
            Some((_, current)) => {
                *current = current
                    .checked_add(&snapshot.quantity_total)
                    .map_err(|_| RecipeError::QuantityOutOfRange)?;
            }
            None => {
                totals.insert(
                    snapshot.inventory_item_id.clone(),
                    (snapshot.base_unit, snapshot.quantity_total),
                );
            }
        }
    }
    Ok(totals
        .into_iter()
        .map(
            |(inventory_item_id, (base_unit, quantity))| ResolvedConsumptionComponent {
                inventory_item_id,
                base_unit,
                quantity,
            },
        )
        .collect())
}
