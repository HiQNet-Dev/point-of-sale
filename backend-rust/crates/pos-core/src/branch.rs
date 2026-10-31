//! Agregado puro de sucursal y herencia de zona horaria.

use crate::organization::{
    BranchId, BusinessCode, ChangedFields, DisplayName, DomainError, IanaTimezone, OrganizationId,
    Revision,
};

/// Estado operacional de una sucursal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchStatus {
    /// La sucursal está habilitada para operaciones sujetas a autorización externa.
    Active,
    /// La sucursal conserva datos históricos pero no acepta operaciones nuevas.
    Inactive,
}

/// Cambio de override de zona en un patch de sucursal.
///
/// Modela la distinción Java entre campo omitido y `null` explícito.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TimezonePatch {
    /// Conserva el override actual.
    #[default]
    Keep,
    /// Fija un override IANA propio.
    Set(IanaTimezone),
    /// Elimina el override; la sucursal vuelve a heredar de la organización.
    ClearToInherited,
}

/// Campos mutables de Branch; ID, owner y código no son reasignables.
#[derive(Debug, Default)]
pub struct BranchPatch {
    /// Nuevo nombre; `None` significa no modificar.
    pub name: Option<DisplayName>,
    /// Cambio explícito del override, con semántica ternaria.
    pub timezone: TimezonePatch,
    /// Nuevo estado; `None` significa no modificar.
    pub status: Option<BranchStatus>,
}

/// Agregado de sucursal sin persistencia, RBAC, auditoría ni reloj.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    id: BranchId,
    organization_id: OrganizationId,
    name: DisplayName,
    code: BusinessCode,
    timezone_override: Option<IanaTimezone>,
    status: BranchStatus,
    revision: Revision,
}

impl Branch {
    /// Crea una sucursal ACTIVE asociada de forma inmutable a una organización.
    ///
    /// La unicidad `(organization_id, code)` es una responsabilidad de la capa de
    /// persistencia y su constraint SQL; este agregado no simula un índice global.
    pub fn create(
        id: BranchId,
        organization_id: OrganizationId,
        name: DisplayName,
        code: BusinessCode,
        timezone_override: Option<IanaTimezone>,
    ) -> Self {
        Self {
            id,
            organization_id,
            name,
            code,
            timezone_override,
            status: BranchStatus::Active,
            revision: Revision::INITIAL,
        }
    }

    /// Devuelve el override o la zona de su organización si no existe override.
    pub fn effective_timezone<'a>(
        &'a self,
        organization_timezone: &'a IanaTimezone,
    ) -> &'a IanaTimezone {
        self.timezone_override
            .as_ref()
            .unwrap_or(organization_timezone)
    }

    /// Aplica un patch si la revisión coincide y conserva ownership/código.
    pub fn apply_patch(
        &mut self,
        expected_revision: Revision,
        patch: BranchPatch,
    ) -> Result<ChangedFields, DomainError> {
        if self.revision != expected_revision {
            return Err(DomainError::VersionMismatch);
        }
        let mut candidate = self.clone();
        let mut changed = Vec::new();
        if let Some(name) = patch.name
            && candidate.name != name
        {
            candidate.name = name;
            changed.push("name");
        }
        match patch.timezone {
            TimezonePatch::Keep => {}
            TimezonePatch::Set(timezone)
                if candidate.timezone_override.as_ref() != Some(&timezone) =>
            {
                candidate.timezone_override = Some(timezone);
                changed.push("timezone");
            }
            TimezonePatch::ClearToInherited if candidate.timezone_override.is_some() => {
                candidate.timezone_override = None;
                changed.push("timezone");
            }
            TimezonePatch::Set(_) | TimezonePatch::ClearToInherited => {}
        }
        if let Some(status) = patch.status
            && candidate.status != status
        {
            candidate.status = status;
            changed.push("status");
        }
        if !changed.is_empty() {
            candidate.revision = Revision::try_new(
                candidate
                    .revision
                    .value()
                    .checked_add(1)
                    .ok_or(DomainError::VersionOverflow)?,
            )?;
            *self = candidate;
        }
        Ok(ChangedFields(changed))
    }

    /// Identidad estable de sucursal.
    pub const fn id(&self) -> BranchId {
        self.id
    }
    /// Organización propietaria inmutable.
    pub const fn organization_id(&self) -> OrganizationId {
        self.organization_id
    }
    /// Nombre visible validado.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }
    /// Código de negocio inmutable.
    pub fn code(&self) -> &BusinessCode {
        &self.code
    }
    /// Override de zona; `None` significa herencia, no zona desconocida.
    pub fn timezone_override(&self) -> Option<&IanaTimezone> {
        self.timezone_override.as_ref()
    }
    /// Estado operacional de la sucursal.
    pub const fn status(&self) -> BranchStatus {
        self.status
    }
    /// Revisión optimista del estado en memoria.
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    /// Indica si está ACTIVE; permisos y excepciones de recuperación son externos.
    pub const fn is_active(&self) -> bool {
        matches!(self.status, BranchStatus::Active)
    }

    #[cfg(test)]
    pub(crate) fn set_revision_for_test(&mut self, revision: Revision) {
        self.revision = revision;
    }
}
