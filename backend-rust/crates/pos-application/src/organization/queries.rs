//! Lectura de sucursales para consumidores autenticados; no modifica bootstrap.

use pos_core::{
    identity::VerifiedPrincipal,
    organization_branch::{ApplicationError, Branch, EntityId, Permission},
};

/// Puerto de lectura scoped, consumido por el listado administrativo.
#[allow(async_fn_in_trait)]
pub trait BranchReader {
    /// Devuelve exclusivamente sucursales de la organización indicada.
    async fn read_branches(&self, organization: &EntityId)
    -> Result<Vec<Branch>, ApplicationError>;
}

/// Consulta de aplicación: el owner siempre procede del principal verificado.
pub async fn list_authorized_branches<R: BranchReader>(
    reader: &R,
    principal: &VerifiedPrincipal,
) -> Result<Vec<Branch>, ApplicationError> {
    if principal.scope().branch_id().is_some() {
        return Err(ApplicationError::Forbidden);
    }
    let organization =
        crate::authorization::require_organization_scope(principal, Permission::BranchRead)?;
    reader.read_branches(&organization).await
}
