//! Roles de organización: agrupación nombrada de permisos.
//!
//! El rol no concede nada por sí mismo: los permisos viven en la tabla de unión
//! `role_permission` y solo producen efecto cuando un rol ACTIVO está asignado a
//! un usuario ACTIVO dentro de una organización ACTIVA. Esta pieza modela
//! únicamente identidad, nombre, estado y revisión optimista.
//!
//! # Invariantes
//!
//! - El código es inmutable después del alta y único dentro de la organización.
//! - La identidad y el propietario no se pueden modificar.
//! - Toda mutación exige la revisión leída previamente.
//! - Un patch sin cambios materiales no avanza la revisión.

use crate::organization_branch::{BusinessCode, DisplayName, EntityId, Status};

use super::IdentityError;

/// Cambios permitidos sobre un rol existente.
///
/// Ni la identidad, ni el propietario, ni el código forman parte del contrato.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RolePatch {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nombre visible nuevo, si cambia.
    pub name: Option<DisplayName>,
    /// Estado administrativo nuevo, si cambia.
    pub status: Option<Status>,
}

/// Rol persistido de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Role {
    id: EntityId,
    organization_id: EntityId,
    code: BusinessCode,
    name: DisplayName,
    status: Status,
    revision: u64,
}

impl Role {
    /// Crea un rol activo con revisión inicial cero.
    pub fn create(
        id: EntityId,
        organization_id: EntityId,
        code: BusinessCode,
        name: DisplayName,
    ) -> Self {
        Self {
            id,
            organization_id,
            code,
            name,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata un rol persistido conservando estado y revisión.
    ///
    /// Uso exclusivo de adaptadores de persistencia: los value objects ya llegan
    /// validados.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: BusinessCode,
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

    /// Propietario inmutable.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Código inmutable y único dentro de la organización.
    pub fn code(&self) -> &BusinessCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Estado administrativo.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista de la fila.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el rol puede aportar permisos.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Aplica un patch atómico y devuelve si hubo cambios materiales.
    ///
    /// # Errors
    /// Devuelve [`IdentityError::ConcurrentModification`] si la revisión no
    /// coincide y [`IdentityError::RevisionOverflow`] si no puede avanzar.
    pub fn patch(&mut self, patch: RolePatch) -> Result<bool, IdentityError> {
        if patch.expected_revision != self.revision {
            return Err(IdentityError::ConcurrentModification);
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
                .ok_or(IdentityError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }

    /// Valida la revisión y avanza el agregado al reemplazar sus permisos.
    ///
    /// La pertenencia de los permisos vive en la tabla de unión; el agregado solo
    /// conserva la revisión que protege la operación completa.
    ///
    /// # Errors
    /// Devuelve [`IdentityError::ConcurrentModification`] si la revisión no
    /// coincide y [`IdentityError::RevisionOverflow`] si no puede avanzar.
    pub fn touch(&mut self, expected_revision: u64) -> Result<(), IdentityError> {
        if expected_revision != self.revision {
            return Err(IdentityError::ConcurrentModification);
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(IdentityError::RevisionOverflow)?;
        Ok(())
    }
}

/// Campos materiales que difieren entre dos revisiones del mismo rol.
pub fn role_changed_fields(before: &Role, after: &Role) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    fields
}
