use super::*;
use crate::organization_branch::{DisplayName, EntityId, Status};

/// Datos de creación de una categoría.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCategory {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Código ya validado.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
}

/// Categoría de producto propiedad de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Category {
    id: EntityId,
    organization_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    status: Status,
    revision: u64,
}

impl Category {
    /// Crea una categoría activa con revisión `0`.
    pub fn create(new: NewCategory) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            code: new.code,
            name: new.name,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata una categoría persistida.
    ///
    /// Uso exclusivo de adaptadores: el código y el nombre ya llegan validados.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            code,
            name,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria; el código solo es único dentro de ella.
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

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si la categoría admite nuevas asignaciones de producto.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`] si la versión esperada
    /// no coincide y [`CatalogError::RevisionOverflow`] si la revisión no cabe.
    pub fn patch(&mut self, patch: CategoryPatch) -> Result<bool, CatalogError> {
        if patch.expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
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

/// Patch de categoría: el código y la organización nunca se modifican.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Campos modificados de una categoría, con los nombres de la auditoría Java.
pub fn category_changed_fields(before: &Category, after: &Category) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}
