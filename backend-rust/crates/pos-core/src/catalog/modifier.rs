use super::*;
use crate::organization_branch::{DisplayName, EntityId, Status};

/// Datos de creación de un grupo de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewModifierGroup {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Código ya validado.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
    /// Mínimo de opciones exigidas (`>= 0`).
    pub min_selections: u32,
    /// Máximo de opciones admitidas (`>= 1`).
    pub max_selections: u32,
}

/// Grupo de modificadores reutilizable propiedad de una organización.
///
/// Los semánticos `required`/`single`/`multi` se derivan de los límites y nunca
/// se almacenan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierGroup {
    id: EntityId,
    organization_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    min_selections: u32,
    max_selections: u32,
    status: Status,
    revision: u64,
}

impl ModifierGroup {
    /// Crea un grupo activo con revisión `0`.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidSelectionRange`] si `max_selections` es
    /// cero o `min_selections` lo supera.
    pub fn create(new: NewModifierGroup) -> Result<Self, CatalogError> {
        validate_selections(new.min_selections, new.max_selections)?;
        Ok(Self {
            id: new.id,
            organization_id: new.organization_id,
            code: new.code,
            name: new.name,
            min_selections: new.min_selections,
            max_selections: new.max_selections,
            status: Status::Active,
            revision: 0,
        })
    }

    /// Rehidrata un grupo persistido validando sus límites.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidSelectionRange`] si la fila es incoherente.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        min_selections: u32,
        max_selections: u32,
        status: Status,
        revision: u64,
    ) -> Result<Self, CatalogError> {
        validate_selections(min_selections, max_selections)?;
        Ok(Self {
            id,
            organization_id,
            code,
            name,
            min_selections,
            max_selections,
            status,
            revision,
        })
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Código inmutable.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Mínimo de opciones exigidas.
    pub fn min_selections(&self) -> u32 {
        self.min_selections
    }

    /// Máximo de opciones admitidas.
    pub fn max_selections(&self) -> u32 {
        self.max_selections
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el grupo es seleccionable en operación.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Derivado: al menos una opción es obligatoria.
    pub fn is_required(&self) -> bool {
        self.min_selections > 0
    }

    /// Derivado: exactamente una opción.
    pub fn is_single_choice(&self) -> bool {
        self.max_selections == 1
    }

    /// Derivado: más de una opción.
    pub fn is_multiple_choice(&self) -> bool {
        self.max_selections > 1
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`],
    /// [`CatalogError::InvalidSelectionRange`] o [`CatalogError::RevisionOverflow`].
    pub fn patch(&mut self, patch: ModifierGroupPatch) -> Result<bool, CatalogError> {
        if patch.expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        if patch.min_selections.is_some() || patch.max_selections.is_some() {
            let min = patch.min_selections.unwrap_or(next.min_selections);
            let max = patch.max_selections.unwrap_or(next.max_selections);
            validate_selections(min, max)?;
            changed |= next.min_selections != min || next.max_selections != max;
            next.min_selections = min;
            next.max_selections = max;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(CatalogError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch de grupo de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierGroupPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Mínimo nuevo; si se omite se conserva el vigente.
    pub min_selections: Option<u32>,
    /// Máximo nuevo; si se omite se conserva el vigente.
    pub max_selections: Option<u32>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Datos de creación de una opción de modificador.
///
/// La organización se deriva del grupo: una opción nunca puede pertenecer a una
/// organización distinta de la de su grupo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewModifierOption {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Grupo propietario ya resuelto y de la organización solicitada.
    pub group: ModifierGroup,
    /// Código ya validado; único dentro del grupo.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
    /// Delta firmado sobre el precio base.
    pub price_delta: PriceDelta,
    /// Orden de presentación dentro del grupo.
    pub display_order: u32,
}

/// Opción de un grupo de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierOption {
    id: EntityId,
    organization_id: EntityId,
    group_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    price_delta: PriceDelta,
    display_order: u32,
    status: Status,
    revision: u64,
}

impl ModifierOption {
    /// Crea una opción activa con revisión `0`.
    ///
    /// La actividad del grupo no se exige, igual que en el agregado Java: solo
    /// se comprueba la pertenencia a la organización, que aquí se deriva del
    /// propio grupo.
    pub fn create(new: NewModifierOption) -> Self {
        Self {
            id: new.id,
            organization_id: new.group.organization_id().clone(),
            group_id: new.group.id().clone(),
            code: new.code,
            name: new.name,
            price_delta: new.price_delta,
            display_order: new.display_order,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata una opción persistida.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        group_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        price_delta: PriceDelta,
        display_order: u32,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            group_id,
            code,
            name,
            price_delta,
            display_order,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria, derivada del grupo.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Grupo propietario.
    pub fn group_id(&self) -> &EntityId {
        &self.group_id
    }

    /// Código inmutable y único dentro del grupo.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Delta firmado sobre el precio base.
    pub fn price_delta(&self) -> PriceDelta {
        self.price_delta
    }

    /// Orden de presentación.
    pub fn display_order(&self) -> u32 {
        self.display_order
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si la opción es seleccionable en operación.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`] o
    /// [`CatalogError::RevisionOverflow`].
    pub fn patch(&mut self, patch: ModifierOptionPatch) -> Result<bool, CatalogError> {
        if patch.expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        if let Some(value) = patch.price_delta {
            changed |= next.price_delta != value;
            next.price_delta = value;
        }
        if let Some(value) = patch.display_order {
            changed |= next.display_order != value;
            next.display_order = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(CatalogError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch de opción de modificador.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierOptionPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Delta nuevo.
    pub price_delta: Option<PriceDelta>,
    /// Orden nuevo.
    pub display_order: Option<u32>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Resultado puro del reemplazo de la lista de grupos de un producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModifierReplacement {
    /// La lista solicitada coincide con la vigente: sin mutación ni auditoría.
    Unchanged,
    /// Configuración nueva en el orden solicitado.
    Replace(Vec<EntityId>),
}

/// Calcula el reemplazo ordenado de la configuración de modificadores.
///
/// La posición en la lista es el orden de presentación. Una lista idéntica es
/// una operación nula; una lista con identificadores repetidos se rechaza.
///
/// # Errors
/// Devuelve [`CatalogError::DuplicateModifierGroup`] si hay identificadores
/// repetidos.
pub fn plan_modifier_replacement(
    current: &[EntityId],
    requested: &[EntityId],
) -> Result<ModifierReplacement, CatalogError> {
    let mut seen = std::collections::BTreeSet::new();
    for group in requested {
        if !seen.insert(group) {
            return Err(CatalogError::DuplicateModifierGroup);
        }
    }
    if current == requested {
        return Ok(ModifierReplacement::Unchanged);
    }
    Ok(ModifierReplacement::Replace(requested.to_vec()))
}

/// Campos modificados de un grupo de modificadores.
pub fn modifier_group_changed_fields(
    before: &ModifierGroup,
    after: &ModifierGroup,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.min_selections != after.min_selections
        || before.max_selections != after.max_selections
    {
        changed.push("selections");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}

/// Campos modificados de una opción de modificador.
pub fn modifier_option_changed_fields(
    before: &ModifierOption,
    after: &ModifierOption,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.price_delta != after.price_delta {
        changed.push("priceDelta");
    }
    if before.display_order != after.display_order {
        changed.push("displayOrder");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}

fn validate_selections(min: u32, max: u32) -> Result<(), CatalogError> {
    if max == 0 || min > max {
        return Err(CatalogError::InvalidSelectionRange);
    }
    Ok(())
}
