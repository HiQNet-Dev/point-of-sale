use crate::organization_branch::EntityId;
use crate::organization_branch::{DisplayName, java_trim};

use super::*;

/// Email normalizado como Java: `String.trim()` y minúsculas con semántica Root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedEmail(String);

impl NormalizedEmail {
    /// Valida la forma mínima heredada (`local@domain.tld`) y 254 unidades UTF-16.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        let normalized = java_trim(value).to_lowercase();
        let mut parts = normalized.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        if parts.next().is_some()
            || local.is_empty()
            || domain.is_empty()
            || !domain.contains('.')
            || normalized.chars().any(char::is_whitespace)
            || normalized.contains('\0')
            || normalized.encode_utf16().count() > 254
        {
            return Err(IdentityError::InvalidEmail);
        }
        Ok(Self(normalized))
    }

    /// Representación segura para búsquedas y unicidad por Organization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Estado administrativo del usuario Local.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PosUserStatus {
    /// Usuario disponible para autenticación y autorización.
    Active,
    /// Usuario conservado históricamente, sin capacidad operativa.
    Inactive,
}

/// Patch permitido: ni UUID ni Organization pueden modificarse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosUserPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre visible opcional.
    pub display_name: Option<DisplayName>,
    /// Email opcional ya validado.
    pub email: Option<NormalizedEmail>,
    /// Estado administrativo opcional.
    pub status: Option<PosUserStatus>,
}

/// Usuario Local, propietario de una sola Organization e independiente de credenciales.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosUser {
    id: EntityId,
    organization_id: EntityId,
    display_name: DisplayName,
    email: NormalizedEmail,
    status: PosUserStatus,
    revision: u64,
}

impl PosUser {
    /// Crea una identidad activa; credenciales se asocian posteriormente por UUID.
    pub fn create(
        id: EntityId,
        organization_id: EntityId,
        display_name: DisplayName,
        email: NormalizedEmail,
    ) -> Self {
        Self {
            id,
            organization_id,
            display_name,
            email,
            status: PosUserStatus::Active,
            revision: 0,
        }
    }

    /// Rehidrata un usuario persistido conservando estado y revisión.
    ///
    /// Uso exclusivo de adaptadores de persistencia; el email y el nombre ya
    /// llegan validados por sus value objects.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        display_name: DisplayName,
        email: NormalizedEmail,
        status: PosUserStatus,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            display_name,
            email,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Owner inmutable.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Nombre visible no secreto.
    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }
    /// Email normalizado.
    pub fn email(&self) -> &NormalizedEmail {
        &self.email
    }
    /// Estado actual.
    pub fn status(&self) -> PosUserStatus {
        self.status
    }
    /// Revisión optimista de la fila de usuario Java (`users.version`).
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Indica si la identidad puede autenticarse/autorizarse.
    pub fn is_active(&self) -> bool {
        self.status == PosUserStatus::Active
    }

    /// Aplica un patch atómico sin permitir mover al usuario ni modificar su UUID.
    pub fn patch(&mut self, patch: PosUserPatch) -> Result<bool, IdentityError> {
        if patch.expected_version != self.revision {
            return Err(IdentityError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.display_name {
            changed |= next.display_name != value;
            next.display_name = value;
        }
        if let Some(value) = patch.email {
            changed |= next.email != value;
            next.email = value;
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
}
