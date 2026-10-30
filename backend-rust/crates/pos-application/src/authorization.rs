//! Autorización contextual compartida por los casos de uso.
//!
//! Estas funciones no consultan la base de datos: reciben un
//! [`VerifiedPrincipal`] ya resuelto y comprueban el permiso contra el scope
//! exacto. La resolución DB-authoritative de permisos permanece en el adaptador
//! de identidad; aquí solo se decide.
//!
//! # Invariantes
//!
//! - Un permiso resuelto para la sucursal A nunca autoriza la sucursal B.
//! - El alcance de organización exige el scope exacto del principal; no se
//!   amplía a `branch = None`.
//! - El modo recuperación solo se admite cuando el caso de uso lo declara.

use pos_core::{
    identity::{PermissionCode, PrincipalScope, ScopeMode, VerifiedPrincipal},
    organization_branch::{ApplicationError, EntityId, Permission},
    sales::SaleScope,
};

/// Exige modo normal salvo que el caso de uso autorice recuperación.
fn require_mode(
    principal: &VerifiedPrincipal,
    allow_recovery: bool,
) -> Result<(), ApplicationError> {
    if !allow_recovery && principal.scope().mode() != ScopeMode::Normal {
        return Err(ApplicationError::Forbidden);
    }
    Ok(())
}

/// Traduce el permiso de aplicación a su código canónico y lo verifica contra
/// el scope indicado.
fn authorize_scope(
    principal: &VerifiedPrincipal,
    permission: Permission,
    scope: &PrincipalScope,
) -> Result<(), ApplicationError> {
    let code = PermissionCode::new(permission.as_code().ok_or(ApplicationError::Forbidden)?)
        .map_err(|_| ApplicationError::Forbidden)?;
    principal
        .authorize(&code, scope)
        .map_err(|_| ApplicationError::Forbidden)
}

/// Autoriza un permiso de alcance de organización contra el scope del principal.
///
/// Devuelve la organización propietaria. La comprobación usa el scope exacto del
/// principal para no convertir un permiso de sucursal en uno global.
///
/// # Errors
/// Devuelve [`ApplicationError::Forbidden`] en modo recuperación o sin permiso.
pub fn require_organization_scope(
    principal: &VerifiedPrincipal,
    permission: Permission,
) -> Result<EntityId, ApplicationError> {
    require_mode(principal, false)?;
    authorize_scope(principal, permission, principal.scope())?;
    Ok(principal.scope().organization_id().clone())
}

/// Autoriza un permiso de sucursal y devuelve `(organización, sucursal)`.
///
/// La sucursal procede siempre del principal verificado, nunca de la entrada.
///
/// # Errors
/// Devuelve [`ApplicationError::Forbidden`] si no hay sucursal, el modo no
/// corresponde o falta el permiso.
pub fn require_branch_scope(
    principal: &VerifiedPrincipal,
    permission: Permission,
    allow_recovery: bool,
) -> Result<(EntityId, EntityId), ApplicationError> {
    require_mode(principal, allow_recovery)?;
    let organization_id = principal.scope().organization_id().clone();
    let branch_id = principal
        .scope()
        .branch_id()
        .cloned()
        .ok_or(ApplicationError::Forbidden)?;
    let scope = if principal.scope().mode() == ScopeMode::Recovery {
        PrincipalScope::recovery(organization_id.clone(), branch_id.clone())
    } else {
        PrincipalScope::new(organization_id.clone(), Some(branch_id.clone()))
    };
    authorize_scope(principal, permission, &scope)?;
    Ok((organization_id, branch_id))
}

/// Autoriza un permiso operativo y construye el scope completo de una venta.
///
/// Solo el permiso se decide aquí; la existencia y el estado de sucursal, caja y
/// turno los verifica [`crate::sales_context`] dentro de la misma transacción.
///
/// # Errors
/// Devuelve [`ApplicationError::Forbidden`] sin sucursal o sin permiso.
#[allow(clippy::too_many_arguments)]
pub fn require_sale_scope(
    principal: &VerifiedPrincipal,
    permission: Permission,
    allow_recovery: bool,
    cash_register_id: EntityId,
    cash_shift_id: EntityId,
) -> Result<SaleScope, ApplicationError> {
    let (organization_id, branch_id) = require_branch_scope(principal, permission, allow_recovery)?;
    Ok(SaleScope {
        organization_id,
        branch_id,
        cash_register_id,
        cash_shift_id,
    })
}

/// Devuelve la sucursal operativa del principal o falla cerrado.
///
/// # Errors
/// Devuelve [`ApplicationError::Forbidden`] si el principal no tiene sucursal.
pub fn require_operational_branch(
    principal: &VerifiedPrincipal,
) -> Result<EntityId, ApplicationError> {
    principal
        .scope()
        .branch_id()
        .cloned()
        .ok_or(ApplicationError::Forbidden)
}
