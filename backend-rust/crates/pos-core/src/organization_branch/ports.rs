use super::*;

/// Reloj inyectable para adaptadores que deban sellar operaciones en UTC.
pub trait Clock {
    /// Devuelve el instante actual. Un fallo se propaga y debe abortar la UoW.
    fn now_utc(&self) -> Result<UtcTimestamp, ApplicationError>;
}

/// Dependencia para generar UUIDs; una implementación productiva no puede reutilizar IDs.
pub trait IdentityGenerator {
    /// Genera una identidad UUID canónica nueva.
    fn next_id(&mut self) -> Result<EntityId, ApplicationError>;
}

/// Decisión contextual; toda implementación debe denegar al no poder decidir.
pub trait Authorization {
    /// Decide un permiso contra su contexto de organización y sucursal opcional.
    fn is_allowed(
        &self,
        permission: Permission,
        organization_id: Option<&EntityId>,
        branch_id: Option<&EntityId>,
    ) -> bool;
}

/// Repositorio de organización expresado en operaciones del negocio.
pub trait OrganizationRepository {
    /// Busca por identidad.
    fn get_organization(&mut self, id: &EntityId)
    -> Result<Option<Organization>, ApplicationError>;
    /// Indica si el código global ya está ocupado.
    fn organization_code_exists(&mut self, code: &BusinessCode) -> Result<bool, ApplicationError>;
    /// Inserta una organización nueva.
    fn insert_organization(&mut self, organization: &Organization) -> Result<(), ApplicationError>;
    /// Actualiza condicionalmente por revisión previa; `false` representa CAS perdido.
    fn update_organization_cas(
        &mut self,
        previous_revision: u64,
        organization: &Organization,
    ) -> Result<bool, ApplicationError>;
}

/// Repositorio de sucursal siempre scoped por organización.
pub trait BranchRepository {
    /// Busca una sucursal dentro de su owner; jamás recibe lookup global.
    fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Option<Branch>, ApplicationError>;
    /// Lista solo sucursales del owner solicitado.
    fn list_branches(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Vec<Branch>, ApplicationError>;
    /// Comprueba unicidad del código dentro del owner.
    fn branch_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &BusinessCode,
    ) -> Result<bool, ApplicationError>;
    /// Inserta una sucursal nueva.
    fn insert_branch(&mut self, branch: &Branch) -> Result<(), ApplicationError>;
    /// CAS scoped por owner y revisión previa.
    fn update_branch_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        branch: &Branch,
    ) -> Result<bool, ApplicationError>;
}

/// Auditoría append-only que se ejecuta en la transacción actual.
pub trait AuditAppender {
    /// Añade un evento o falla; el fallo debe abortar la unidad de trabajo.
    fn append_audit(&mut self, event: AuditEvent) -> Result<(), ApplicationError>;
}

/// Contexto mutante transaccional de los casos de uso.
pub trait OrganizationBranchSession:
    OrganizationRepository + BranchRepository + AuditAppender
{
}
impl<T: OrganizationRepository + BranchRepository + AuditAppender> OrganizationBranchSession for T {}

/// Unidad de trabajo sin acoplarla a SQLx ni a un framework.
pub trait UnitOfWork {
    /// Sesión que comparte la transacción de negocio y auditoría.
    type Session: OrganizationBranchSession;
    /// Ejecuta y confirma solo cuando la operación completa devuelve éxito.
    fn transaction<T>(
        &mut self,
        operation: impl FnOnce(&mut Self::Session) -> Result<T, ApplicationError>,
    ) -> Result<T, ApplicationError>;
}
