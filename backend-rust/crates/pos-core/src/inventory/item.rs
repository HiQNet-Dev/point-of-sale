use super::*;
use crate::catalog::{CatalogCode, Description, PatchField};
use crate::organization_branch::{DisplayName, EntityId, Status};

/// Datos de creación de un artículo de inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInventoryItem {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Código ya validado; único por organización e inmutable.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
    /// Descripción opcional ya normalizada.
    pub description: Option<Description>,
    /// Unidad base de todas las cantidades del artículo.
    pub base_unit: InventoryBaseUnit,
    /// Si el artículo admite cantidades fraccionarias.
    pub fractional_unit_allowed: bool,
}

/// Artículo de inventario propiedad de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryItem {
    id: EntityId,
    organization_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    description: Option<Description>,
    base_unit: InventoryBaseUnit,
    fractional_unit_allowed: bool,
    unit_locked: bool,
    status: Status,
    revision: u64,
}

impl InventoryItem {
    /// Crea un artículo activo, con unidad libre y revisión `0`.
    pub fn create(new: NewInventoryItem) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            code: new.code,
            name: new.name,
            description: new.description,
            base_unit: new.base_unit,
            fractional_unit_allowed: new.fractional_unit_allowed,
            unit_locked: false,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata un artículo persistido.
    ///
    /// Uso exclusivo de adaptadores: los valores ya llegan validados.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        description: Option<Description>,
        base_unit: InventoryBaseUnit,
        fractional_unit_allowed: bool,
        unit_locked: bool,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            code,
            name,
            description,
            base_unit,
            fractional_unit_allowed,
            unit_locked,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Código inmutable y único por organización.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Descripción opcional.
    pub fn description(&self) -> Option<&Description> {
        self.description.as_ref()
    }

    /// Unidad base de las cantidades.
    pub fn base_unit(&self) -> InventoryBaseUnit {
        self.base_unit
    }

    /// Indica si el artículo admite cantidades fraccionarias.
    pub fn fractional_unit_allowed(&self) -> bool {
        self.fractional_unit_allowed
    }

    /// Indica si la unidad base quedó congelada por dependencias.
    pub fn unit_locked(&self) -> bool {
        self.unit_locked
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el artículo admite mutaciones de stock.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Congela la unidad base de forma monótona.
    ///
    /// No incrementa la revisión: es un marcador persistido por una sentencia
    /// dedicada, no una modificación de negocio. Nunca vuelve a `false`.
    pub fn mark_unit_locked(&mut self) -> bool {
        let changed = !self.unit_locked;
        self.unit_locked = true;
        changed
    }

    /// Comprueba que la unidad admita la cantidad indicada.
    ///
    /// Un artículo `UNIT` sin ``fractional_unit_allowed`` exige cantidad entera.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::FractionalQuantityNotAllowed`].
    pub fn ensure_quantity_fits_unit(&self, quantity: Quantity) -> Result<(), InventoryError> {
        if self.base_unit == InventoryBaseUnit::Unit
            && !self.fractional_unit_allowed
            && quantity.is_fractional()
        {
            return Err(InventoryError::FractionalQuantityNotAllowed);
        }
        Ok(())
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// El código y la organización no son modificables. Cambiar la unidad base
    /// exige que siga libre.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::UnitLocked`],
    /// [`InventoryError::ConcurrentModification`] o
    /// [`InventoryError::RevisionOverflow`].
    pub fn patch(&mut self, patch: InventoryItemPatch) -> Result<bool, InventoryError> {
        if patch.expected_version != self.revision {
            return Err(InventoryError::ConcurrentModification);
        }
        if let Some(base_unit) = patch.base_unit
            && base_unit != self.base_unit
            && self.unit_locked
        {
            return Err(InventoryError::UnitLocked);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        match patch.description {
            PatchField::Absent => {}
            PatchField::Clear => {
                changed |= next.description.is_some();
                next.description = None;
            }
            PatchField::Set(value) => {
                changed |= next.description.as_ref() != Some(&value);
                next.description = Some(value);
            }
        }
        if let Some(value) = patch.base_unit {
            changed |= next.base_unit != value;
            next.base_unit = value;
        }
        if let Some(value) = patch.fractional_unit_allowed {
            changed |= next.fractional_unit_allowed != value;
            next.fractional_unit_allowed = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(InventoryError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch de artículo de inventario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryItemPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Descripción: ausente, limpiada o nueva.
    pub description: PatchField<Description>,
    /// Unidad base nueva; solo si sigue libre.
    pub base_unit: Option<InventoryBaseUnit>,
    /// Autorización de fracciones nueva.
    pub fractional_unit_allowed: Option<bool>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Campos modificados de un artículo, con los nombres de la auditoría Java.
pub fn inventory_item_changed_fields(
    before: &InventoryItem,
    after: &InventoryItem,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.description != after.description {
        changed.push("description");
    }
    if before.base_unit != after.base_unit {
        changed.push("baseUnit");
    }
    if before.fractional_unit_allowed != after.fractional_unit_allowed {
        changed.push("fractionalUnitAllowed");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}
