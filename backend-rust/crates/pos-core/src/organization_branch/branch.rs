use super::*;

/// Valor triestado del override de zona de una sucursal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimezoneOverridePatch {
    /// El campo fue omitido y no cambia.
    Keep,
    /// Define un override explícito.
    Set(Timezone),
    /// Elimina el override y hereda de la organización.
    Clear,
}

/// Datos de creación de una sucursal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewBranch {
    /// Nombre visible.
    pub name: DisplayName,
    /// Código único dentro del owner.
    pub code: BusinessCode,
    /// Override opcional; `None` hereda.
    pub timezone_override: Option<Timezone>,
}

/// Agregado de sucursal, siempre propiedad de una única organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    id: EntityId,
    organization_id: EntityId,
    name: DisplayName,
    code: BusinessCode,
    timezone_override: Option<Timezone>,
    status: Status,
    revision: u64,
}

impl Branch {
    /// Crea una sucursal activa con revisión cero.
    pub fn create(id: EntityId, organization_id: EntityId, input: NewBranch) -> Self {
        Self {
            id,
            organization_id,
            name: input.name,
            code: input.code,
            timezone_override: input.timezone_override,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Reconstruye una fila validada.
    #[allow(clippy::too_many_arguments)]
    pub fn rehydrate(
        id: EntityId,
        organization_id: EntityId,
        name: DisplayName,
        code: BusinessCode,
        timezone_override: Option<Timezone>,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            name,
            code,
            timezone_override,
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
    /// Código inmutable dentro del owner.
    pub fn code(&self) -> &BusinessCode {
        &self.code
    }
    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }
    /// Override opcional.
    pub fn timezone_override(&self) -> Option<&Timezone> {
        self.timezone_override.as_ref()
    }
    /// Resuelve el override o la zona de la organización recibida.
    pub fn effective_timezone<'a>(&'a self, organization_timezone: &'a Timezone) -> &'a Timezone {
        self.timezone_override
            .as_ref()
            .unwrap_or(organization_timezone)
    }
    /// Estado.
    pub fn status(&self) -> Status {
        self.status
    }
    /// Revisión.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Aplica un patch atómicamente y comprueba explícitamente el owner de contexto.
    pub fn patch(
        &mut self,
        organization_id: &EntityId,
        patch: BranchPatch,
    ) -> Result<bool, DomainError> {
        if &self.organization_id != organization_id {
            return Err(DomainError::OwnershipMismatch);
        }
        if self.revision != patch.expected_revision {
            return Err(DomainError::StaleRevision);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        match patch.timezone_override {
            TimezoneOverridePatch::Keep => {}
            TimezoneOverridePatch::Set(value) => {
                changed |= next.timezone_override.as_ref() != Some(&value);
                next.timezone_override = Some(value);
            }
            TimezoneOverridePatch::Clear => {
                changed |= next.timezone_override.is_some();
                next.timezone_override = None;
            }
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .filter(|revision| *revision <= MAX_REVISION)
                .ok_or(DomainError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch de sucursal: ni identidad, owner ni código pueden modificarse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPatch {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nombre opcional.
    pub name: Option<DisplayName>,
    /// Operación triestado sobre el override.
    pub timezone_override: TimezoneOverridePatch,
    /// Estado opcional.
    pub status: Option<Status>,
}

/// Devuelve los nombres estables de los campos Branch modificados.
///
/// Evita que adaptadores de infraestructura reconstruyan su propia lista de
/// cambios auditables.
pub fn branch_changed_fields(before: &Branch, after: &Branch) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name");
    }
    if before.timezone_override() != after.timezone_override() {
        fields.push("timezone");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    fields
}
