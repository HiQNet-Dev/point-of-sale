//! Membresías de sucursal: vínculo entre un usuario y una sucursal.
//!
//! La membresía es la **única** vía por la que un usuario obtiene permisos de
//! alcance sucursal. Sin una membresía ACTIVA con un rol ACTIVO, un principal
//! con sucursal seleccionada no recibe ningún permiso de esa pierna
//! (`LocalPrincipalService` lo resuelve así en SQLite).
//!
//! # Invariantes
//!
//! - Un usuario tiene como máximo una membresía por sucursal
//!   (`UNIQUE (organization_id, branch_id, user_id)`).
//! - Organización, sucursal y usuario son inmutables: la membresía se crea y se
//!   activa o desactiva, nunca se muda de tenant ni de sucursal.
//! - Toda mutación exige la revisión leída previamente.

use crate::organization_branch::{EntityId, Status};

use super::IdentityError;

/// Cambios permitidos sobre una membresía existente.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipPatch {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Estado administrativo nuevo, si cambia.
    pub status: Option<Status>,
}

/// Vínculo persistido entre un usuario y una sucursal de su organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchMembership {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    user_id: EntityId,
    status: Status,
    revision: u64,
}

impl BranchMembership {
    /// Crea una membresía activa con revisión inicial cero.
    pub fn create(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        user_id: EntityId,
    ) -> Self {
        Self {
            id,
            organization_id,
            branch_id,
            user_id,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata una membresía persistida conservando estado y revisión.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        user_id: EntityId,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            branch_id,
            user_id,
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

    /// Sucursal inmutable.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    /// Usuario inmutable.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }

    /// Estado administrativo.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista de la fila.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si la membresía puede aportar permisos de sucursal.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Aplica un patch atómico y devuelve si hubo cambios materiales.
    ///
    /// # Errors
    /// Devuelve [`IdentityError::ConcurrentModification`] si la revisión no
    /// coincide y [`IdentityError::RevisionOverflow`] si no puede avanzar.
    pub fn patch(&mut self, patch: MembershipPatch) -> Result<bool, IdentityError> {
        if patch.expected_revision != self.revision {
            return Err(IdentityError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
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

    /// Valida la revisión y avanza el agregado al reemplazar sus roles.
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

/// Campos materiales que difieren entre dos revisiones de la misma membresía.
pub fn membership_changed_fields(
    before: &BranchMembership,
    after: &BranchMembership,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.status() != after.status() {
        fields.push("status");
    }
    fields
}
