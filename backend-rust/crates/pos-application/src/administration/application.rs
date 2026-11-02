//! Estructura común de los casos de uso de administración de identidad.
//!
//! Reúne la unidad de trabajo y los auxiliares compartidos por usuarios, roles y
//! membresías: autorización por scope exacto, traducción de errores de dominio y
//! cierre transaccional único.
//!
//! Toda mutación sigue el mismo orden: autorización, lectura, validación de
//! dominio, CAS, auditoría y commit. Un fallo en la auditoría revierte la
//! mutación completa.

use pos_core::{
    identity::{PermissionCode, VerifiedPrincipal},
    organization_branch::{ApplicationError, DomainError, EntityId, Permission},
};

use crate::administration::ports::AdministrationUnitOfWork;

/// Casos de uso de administración de identidad sobre una única unidad de trabajo.
#[derive(Clone)]
pub struct AdministrationApplication<U> {
    unit_of_work: U,
}

impl<U> AdministrationApplication<U>
where
    U: AdministrationUnitOfWork,
{
    /// Ensambla los casos de uso sobre una unidad de trabajo ya configurada.
    pub fn new(unit_of_work: U) -> Self {
        Self { unit_of_work }
    }

    /// Acceso de lectura a la unidad de trabajo para los módulos hermanos.
    pub(super) fn unit_of_work(&self) -> &U {
        &self.unit_of_work
    }
}

/// Exige el permiso para el scope propio del principal y devuelve su organización.
///
/// Se compara contra el scope exacto ya resuelto en SQLite: un permiso obtenido
/// para una sucursal nunca autoriza otra ni un scope más amplio. La organización
/// procede siempre del principal; el transporte no la aporta.
pub(super) fn require_permission(
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

/// Como [`require_permission`], limitando a un principal con sucursal
/// seleccionada a operar únicamente sobre esa sucursal.
pub(super) fn require_branch_permission(
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

/// Traduce un fallo de dominio de identidad al vocabulario de aplicación.
pub(super) fn identity_error(error: pos_core::identity::IdentityError) -> ApplicationError {
    use pos_core::identity::IdentityError;
    match error {
        IdentityError::ConcurrentModification => ApplicationError::ConcurrentModification,
        _ => ApplicationError::InvalidAdministrationInput,
    }
}

/// Traduce un fallo de value object de organización.
pub(super) fn domain_error(error: DomainError) -> ApplicationError {
    match error {
        DomainError::StaleRevision => ApplicationError::ConcurrentModification,
        DomainError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        _ => ApplicationError::InvalidAdministrationInput,
    }
}

/// Parser estricto del estado administrativo de un rol o una membresía.
pub(super) fn parse_status(
    value: &str,
) -> Result<pos_core::organization_branch::Status, ApplicationError> {
    pos_core::organization_branch::Status::parse(value)
        .map_err(|_| ApplicationError::InvalidAdministrationInput)
}

/// Parser estricto del estado administrativo de un usuario.
pub(super) fn parse_user_status(
    value: &str,
) -> Result<pos_core::identity::PosUserStatus, ApplicationError> {
    use pos_core::identity::PosUserStatus;
    match value {
        "ACTIVE" => Ok(PosUserStatus::Active),
        "INACTIVE" => Ok(PosUserStatus::Inactive),
        _ => Err(ApplicationError::InvalidAdministrationInput),
    }
}

/// Parser estricto de una identidad enviada como texto.
pub(super) fn parse_id(value: &str) -> Result<EntityId, ApplicationError> {
    EntityId::parse(value).map_err(|_| ApplicationError::InvalidAdministrationInput)
}

/// Parser estricto de una revisión enviada como texto decimal.
pub(super) fn parse_revision(value: &str) -> Result<u64, ApplicationError> {
    value
        .parse::<u64>()
        .map_err(|_| ApplicationError::InvalidAdministrationInput)
}

/// Confirma la operación o revierte por completo.
pub(super) async fn finish<U, T>(
    application: &AdministrationApplication<U>,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: AdministrationUnitOfWork,
{
    match operation {
        Ok(value) => {
            application
                .unit_of_work()
                .commit_transaction(transaction)
                .await?;
            Ok(value)
        }
        Err(error) => {
            if application
                .unit_of_work()
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
