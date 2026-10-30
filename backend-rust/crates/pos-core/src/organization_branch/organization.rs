use super::*;

/// Datos necesarios para crear una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewOrganization {
    /// Nombre visible.
    pub name: DisplayName,
    /// Código global inmutable.
    pub code: BusinessCode,
    /// Zona horaria raíz.
    pub timezone: Timezone,
    /// Moneda por defecto.
    pub currency: IsoCurrency,
    /// Locale por defecto.
    pub locale: LocaleTag,
}

/// Agregado raíz de configuración de una instalación.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Organization {
    id: EntityId,
    name: DisplayName,
    code: BusinessCode,
    timezone: Timezone,
    currency: IsoCurrency,
    locale: LocaleTag,
    status: Status,
    negative_stock_policy: NegativeStockPolicy,
    revision: u64,
}

impl Organization {
    /// Crea un agregado nuevo con defaults compatibles con Java.
    pub fn create(id: EntityId, input: NewOrganization) -> Self {
        Self {
            id,
            name: input.name,
            code: input.code,
            timezone: input.timezone,
            currency: input.currency,
            locale: input.locale,
            status: Status::Active,
            negative_stock_policy: NegativeStockPolicy::Strict,
            revision: 0,
        }
    }

    /// Reconstruye una fila previamente validada por este módulo.
    #[allow(clippy::too_many_arguments)]
    pub fn rehydrate(
        id: EntityId,
        name: DisplayName,
        code: BusinessCode,
        timezone: Timezone,
        currency: IsoCurrency,
        locale: LocaleTag,
        status: Status,
        negative_stock_policy: NegativeStockPolicy,
        revision: u64,
    ) -> Self {
        Self {
            id,
            name,
            code,
            timezone,
            currency,
            locale,
            status,
            negative_stock_policy,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Código inmutable.
    pub fn code(&self) -> &BusinessCode {
        &self.code
    }
    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }
    /// Zona horaria raíz.
    pub fn timezone(&self) -> &Timezone {
        &self.timezone
    }
    /// Moneda de la organización.
    pub fn currency(&self) -> &IsoCurrency {
        &self.currency
    }
    /// Locale de la organización.
    pub fn locale(&self) -> &LocaleTag {
        &self.locale
    }
    /// Estado operacional.
    pub fn status(&self) -> Status {
        self.status
    }
    /// Política negativa heredada.
    pub fn negative_stock_policy(&self) -> NegativeStockPolicy {
        self.negative_stock_policy
    }
    /// Revisión optimista actual.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Aplica un patch de forma atómica; un error deja el agregado intacto.
    pub fn patch(&mut self, patch: OrganizationPatch) -> Result<bool, DomainError> {
        if self.revision != patch.expected_revision {
            return Err(DomainError::StaleRevision);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        if let Some(value) = patch.timezone {
            changed |= next.timezone != value;
            next.timezone = value;
        }
        if let Some(value) = patch.currency {
            changed |= next.currency != value;
            next.currency = value;
        }
        if let Some(value) = patch.locale {
            changed |= next.locale != value;
            next.locale = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if let Some(value) = patch.negative_stock_policy {
            changed |= next.negative_stock_policy != value;
            next.negative_stock_policy = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(DomainError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Patch parcial; código e identidad no están presentes por diseño.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrganizationPatch {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nuevo nombre opcional.
    pub name: Option<DisplayName>,
    /// Nueva zona horaria opcional.
    pub timezone: Option<Timezone>,
    /// Nueva moneda opcional.
    pub currency: Option<IsoCurrency>,
    /// Nuevo locale opcional.
    pub locale: Option<LocaleTag>,
    /// Nuevo estado opcional.
    pub status: Option<Status>,
    /// Nueva política de stock opcional.
    pub negative_stock_policy: Option<NegativeStockPolicy>,
}

/// Devuelve los nombres estables de los campos Organization modificados.
///
/// Tanto composiciones síncronas como asíncronas deben reutilizar esta función
/// para que la metadata de auditoría conserve paridad semántica con Java.
pub fn organization_changed_fields(
    before: &Organization,
    after: &Organization,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name");
    }
    if before.timezone() != after.timezone() {
        fields.push("timezone");
    }
    if before.currency() != after.currency() {
        fields.push("currency");
    }
    if before.locale() != after.locale() {
        fields.push("locale");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    if before.negative_stock_policy() != after.negative_stock_policy() {
        fields.push("inventoryNegativeStockPolicy");
    }
    fields
}
