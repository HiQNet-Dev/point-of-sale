//! Casos de uso de configuración del negocio.
//!
//! Toda lectura y escritura deriva la organización del [`VerifiedPrincipal`]: el
//! transporte no puede aportar tenant, actor ni permisos.

use pos_core::{
    audit::AuditOccurredAt,
    identity::{PermissionCode, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, Branch, BusinessCode, DisplayName, EntityId, IdentityGenerator,
        LocaleTag, NegativeStockPolicy, NewBranch, Organization, OrganizationPatch, Permission,
        Timezone, TimezoneOverridePatch, branch_changed_fields, organization_changed_fields,
    },
};
use serde_json::json;

use crate::{
    CasOutcome,
    audit::{AuditAppender, append_audit},
    organization::administration::{
        dto::{
            BranchConfiguration, BranchConfigurationUpdate, BranchCreation,
            OrganizationConfigurationUpdate, TimezoneUpdate,
        },
        ports::{OrganizationAdministrationTransaction, OrganizationAdministrationUnitOfWork},
    },
};

/// Capacidad de sucursales de la edición Free Local.
///
/// Se aplica aquí, en la capa de aplicación: la interfaz no puede habilitarla y
/// una edición futura solo tendrá que cambiar este valor y registrar el comando.
pub const FREE_LOCAL_BRANCH_CAPACITY: u64 = 1;

/// Casos de uso de la configuración del negocio autenticado.
#[derive(Clone)]
pub struct OrganizationAdministrationApplication<U> {
    unit_of_work: U,
}

impl<U> OrganizationAdministrationApplication<U>
where
    U: OrganizationAdministrationUnitOfWork,
    U::Transaction: AuditAppender + Send,
{
    /// Ensambla los casos de uso sobre una unidad de trabajo ya configurada.
    pub fn new(unit_of_work: U) -> Self {
        Self { unit_of_work }
    }

    /// Lee la configuración de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `organization.read` y
    /// [`ApplicationError::OrganizationNotFound`] si la organización desapareció.
    pub async fn get_organization(
        &self,
        principal: &VerifiedPrincipal,
    ) -> Result<Organization, ApplicationError> {
        let organization_id = require_permission(principal, Permission::OrganizationRead)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            transaction
                .get_organization(&organization_id)
                .await?
                .ok_or(ApplicationError::OrganizationNotFound)
        }
        .await;
        finish_read(&self.unit_of_work, transaction, operation).await
    }

    /// Actualiza la configuración editable de la organización.
    ///
    /// Solo nombre, zona horaria, configuración regional y política de inventario
    /// negativo forman parte del contrato. Estado y moneda no se pueden expresar:
    /// no existe todavía un contrato de baja ni de reinterpretación monetaria.
    ///
    /// Un patch sin cambios materiales no incrementa la revisión ni audita.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `organization.update`,
    /// [`ApplicationError::InvalidOrganizationInput`] si un valor no cumple el
    /// contrato, [`ApplicationError::ConcurrentModification`] si la revisión
    /// enviada ya no es la vigente y [`ApplicationError::Audit`] si la auditoría
    /// falla, en cuyo caso la mutación se revierte.
    pub async fn update_organization<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: OrganizationConfigurationUpdate,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Organization, ApplicationError> {
        let organization_id = require_permission(principal, Permission::OrganizationUpdate)?;
        let patch = organization_patch(request)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let mut organization = transaction
                .get_organization(&organization_id)
                .await?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            let before = organization.clone();
            let previous_revision = organization.revision();
            if !organization
                .patch(patch)
                .map_err(organization_domain_error)?
            {
                // Sin cambios materiales: ni revisión nueva ni auditoría.
                return Ok(organization);
            }
            if transaction
                .update_organization_cas(previous_revision, &organization, occurred_at.as_str())
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            append_audit(
                &mut transaction,
                identities,
                principal,
                "organization.updated",
                "organization",
                organization.id().clone(),
                json!({ "changedFields": organization_changed_fields(&before, &organization) }),
                occurred_at,
            )
            .await?;
            Ok(organization)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Lista las sucursales de la organización con su zona horaria efectiva.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `branch.read`.
    pub async fn list_branches(
        &self,
        principal: &VerifiedPrincipal,
    ) -> Result<Vec<BranchConfiguration>, ApplicationError> {
        let organization_id = require_permission(principal, Permission::BranchRead)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let organization = transaction
                .get_organization(&organization_id)
                .await?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            let branches = transaction.list_branches(&organization_id).await?;
            Ok(branches
                .into_iter()
                .map(|branch| BranchConfiguration::new(branch, organization.timezone()))
                .collect::<Vec<_>>())
        }
        .await;
        finish_read(&self.unit_of_work, transaction, operation).await
    }

    /// Lee una sucursal de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `branch.read`, o si el
    /// principal está limitado a otra sucursal, y [`ApplicationError::BranchNotFound`]
    /// si la sucursal pertenece a otra organización.
    pub async fn get_branch(
        &self,
        principal: &VerifiedPrincipal,
        branch_id: &EntityId,
    ) -> Result<BranchConfiguration, ApplicationError> {
        let organization_id =
            require_branch_permission(principal, Permission::BranchRead, branch_id)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let organization = transaction
                .get_organization(&organization_id)
                .await?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            let branch = transaction
                .get_branch(&organization_id, branch_id)
                .await?
                .ok_or(ApplicationError::BranchNotFound)?;
            Ok(BranchConfiguration::new(branch, organization.timezone()))
        }
        .await;
        finish_read(&self.unit_of_work, transaction, operation).await
    }

    /// Actualiza nombre y zona horaria de una sucursal existente.
    ///
    /// Identidad, código, propietario y estado no forman parte del contrato.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `branch.update`,
    /// [`ApplicationError::InvalidOrganizationInput`] si un valor no cumple el
    /// contrato, [`ApplicationError::BranchNotFound`] si no existe en la
    /// organización y [`ApplicationError::ConcurrentModification`] con una
    /// revisión obsoleta.
    pub async fn update_branch<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        branch_id: &EntityId,
        request: BranchConfigurationUpdate,
        occurred_at: &AuditOccurredAt,
    ) -> Result<BranchConfiguration, ApplicationError> {
        let organization_id =
            require_branch_permission(principal, Permission::BranchUpdate, branch_id)?;
        let name = request
            .name
            .map(|value| DisplayName::new(&value))
            .transpose()
            .map_err(|_| ApplicationError::InvalidOrganizationInput)?;
        let timezone = timezone_patch(request.timezone)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let organization = transaction
                .get_organization(&organization_id)
                .await?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            let mut branch = transaction
                .get_branch(&organization_id, branch_id)
                .await?
                .ok_or(ApplicationError::BranchNotFound)?;
            let before = branch.clone();
            let previous_revision = branch.revision();
            let patch = pos_core::organization_branch::BranchPatch {
                expected_revision: request.expected_revision,
                name,
                timezone_override: timezone,
                status: None,
            };
            if !branch
                .patch(&organization_id, patch)
                .map_err(branch_domain_error)?
            {
                return Ok(BranchConfiguration::new(branch, organization.timezone()));
            }
            if transaction
                .update_branch_cas(
                    &organization_id,
                    previous_revision,
                    &branch,
                    occurred_at.as_str(),
                )
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            append_audit(
                &mut transaction,
                identities,
                principal,
                "branch.updated",
                "branch",
                branch.id().clone(),
                json!({ "changedFields": branch_changed_fields(&before, &branch) }),
                occurred_at,
            )
            .await?;
            Ok(BranchConfiguration::new(branch, organization.timezone()))
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Crea una sucursal dentro de la capacidad de la edición.
    ///
    /// Free Local v1 admite una única sucursal: la primera alta configura la
    /// sucursal inicial de la instalación y cualquier intento posterior responde
    /// [`ApplicationError::BranchCapacityReached`] aunque el principal tenga
    /// `branch.create`. La regla vive aquí, no en la interfaz.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::BranchCapacityReached`] cuando ya existe una
    /// sucursal, [`ApplicationError::Forbidden`] sin `branch.create`,
    /// [`ApplicationError::BranchCodeAlreadyExists`] si el código está ocupado y
    /// [`ApplicationError::InvalidOrganizationInput`] si los datos no son válidos.
    pub async fn create_branch<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: BranchCreation,
        occurred_at: &AuditOccurredAt,
    ) -> Result<BranchConfiguration, ApplicationError> {
        let organization_id = require_permission(principal, Permission::BranchCreate)?;
        let code = BusinessCode::new(&request.code)
            .map_err(|_| ApplicationError::InvalidOrganizationInput)?;
        let name = DisplayName::new(&request.name)
            .map_err(|_| ApplicationError::InvalidOrganizationInput)?;
        let timezone = request
            .timezone
            .map(|value| Timezone::new(&value))
            .transpose()
            .map_err(|_| ApplicationError::InvalidOrganizationInput)?;
        let branch_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let organization = transaction
                .get_organization(&organization_id)
                .await?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            if transaction.count_branches(&organization_id).await? >= FREE_LOCAL_BRANCH_CAPACITY {
                return Err(ApplicationError::BranchCapacityReached);
            }
            if transaction
                .branch_code_exists(&organization_id, &code)
                .await?
            {
                return Err(ApplicationError::BranchCodeAlreadyExists);
            }
            let branch = Branch::create(
                branch_id,
                organization_id.clone(),
                NewBranch {
                    code,
                    name,
                    timezone_override: timezone,
                },
            );
            transaction
                .insert_branch(&branch, occurred_at.as_str())
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "branch.created",
                "branch",
                branch.id().clone(),
                json!({ "code": branch.code().as_str() }),
                occurred_at,
            )
            .await?;
            Ok(BranchConfiguration::new(branch, organization.timezone()))
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }
}

/// Exige el permiso para el scope propio del principal y devuelve su organización.
///
/// Se compara contra el scope exacto ya resuelto en SQLite, de modo que un
/// permiso obtenido para una sucursal nunca autoriza otra ni un scope más amplio.
/// La organización siempre procede del principal: el transporte no la aporta.
fn require_permission(
    principal: &VerifiedPrincipal,
    permission: Permission,
) -> Result<EntityId, ApplicationError> {
    let code = permission.as_code().ok_or(ApplicationError::Forbidden)?;
    let code = PermissionCode::new(code).map_err(|_| ApplicationError::Forbidden)?;
    principal
        .authorize(&code, principal.scope())
        .map_err(|_| ApplicationError::Forbidden)?;
    Ok(principal.scope().organization_id().clone())
}

/// Como [`require_permission`], limitando además a un principal con sucursal
/// seleccionada a operar únicamente sobre esa sucursal.
fn require_branch_permission(
    principal: &VerifiedPrincipal,
    permission: Permission,
    branch_id: &EntityId,
) -> Result<EntityId, ApplicationError> {
    let organization_id = require_permission(principal, permission)?;
    if let Some(scoped) = principal.scope().branch_id()
        && scoped != branch_id
    {
        return Err(ApplicationError::Forbidden);
    }
    Ok(organization_id)
}

fn organization_patch(
    request: OrganizationConfigurationUpdate,
) -> Result<OrganizationPatch, ApplicationError> {
    let invalid = |_| ApplicationError::InvalidOrganizationInput;
    Ok(OrganizationPatch {
        expected_revision: request.expected_revision,
        name: request
            .name
            .map(|value| DisplayName::new(&value))
            .transpose()
            .map_err(invalid)?,
        timezone: request
            .timezone
            .map(|value| Timezone::new(&value))
            .transpose()
            .map_err(invalid)?,
        // La moneda no forma parte del contrato: cambiarla reinterpretaría
        // importes ya persistidos con otra escala semántica.
        currency: None,
        locale: request
            .locale
            .map(|value| LocaleTag::new(&value))
            .transpose()
            .map_err(invalid)?,
        // El estado no forma parte del contrato: no existe todavía una baja
        // aprobada ni un procedimiento de recuperación para la instalación local.
        status: None,
        negative_stock_policy: request
            .negative_stock_policy
            .map(|value| NegativeStockPolicy::parse(&value))
            .transpose()
            .map_err(invalid)?,
    })
}

fn timezone_patch(update: TimezoneUpdate) -> Result<TimezoneOverridePatch, ApplicationError> {
    match update {
        TimezoneUpdate::Keep => Ok(TimezoneOverridePatch::Keep),
        TimezoneUpdate::Clear => Ok(TimezoneOverridePatch::Clear),
        TimezoneUpdate::Set(value) => Timezone::new(&value)
            .map(TimezoneOverridePatch::Set)
            .map_err(|_| ApplicationError::InvalidOrganizationInput),
    }
}

fn organization_domain_error(
    error: pos_core::organization_branch::DomainError,
) -> ApplicationError {
    use pos_core::organization_branch::DomainError;
    match error {
        DomainError::StaleRevision => ApplicationError::ConcurrentModification,
        DomainError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        _ => ApplicationError::InvalidOrganizationInput,
    }
}

fn branch_domain_error(error: pos_core::organization_branch::DomainError) -> ApplicationError {
    organization_domain_error(error)
}

async fn finish<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: OrganizationAdministrationUnitOfWork,
{
    match operation {
        Ok(value) => {
            unit_of_work.commit_transaction(transaction).await?;
            Ok(value)
        }
        Err(error) => {
            if unit_of_work
                .rollback_transaction(transaction)
                .await
                .is_err()
            {
                return Err(ApplicationError::Storage);
            }
            Err(error)
        }
    }
}

async fn finish_read<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: OrganizationAdministrationUnitOfWork,
{
    // Una lectura no retiene la reserva de escritura más de lo necesario.
    unit_of_work.commit_transaction(transaction).await?;
    operation
}
