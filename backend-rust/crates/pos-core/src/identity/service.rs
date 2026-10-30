use crate::audit::AuditAttributionError;
use crate::organization_branch::EntityId;
use std::collections::BTreeSet;
use std::future::Future;

use super::*;

/// Lecturas DB-authoritative requeridas para construir un principal.
///
/// Los métodos son asíncronos para permitir un adaptador SQLx real sin
/// `block_on` ni conexiones clonadas. El adaptador debe devolver únicamente
/// datos de identidad; nunca hashes, handles crudos ni material criptográfico.
pub trait LocalIdentityRepository {
    /// Busca por hash de sesión ya calculado por el adaptador criptográfico.
    fn find_session_by_hash(
        &mut self,
        hash: &SessionHash,
    ) -> impl Future<Output = Result<Option<LocalSession>, IdentityError>> + Send;
    /// Busca el usuario dentro de la Organization derivada de la sesión.
    fn find_user(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<Option<PosUser>, IdentityError>> + Send;
}

/// Resolución de permisos vigente en DB; no admite grants desde DTO/JWT.
///
/// Contrato alineado con Java:
///
/// - `branch_id = None`: solo grants de alcance organización (rol ACTIVO,
///   asignado al usuario, organización ACTIVA).
/// - `branch_id = Some(b)`: **override de organización** — las grants
///   organizacionales se incluyen siempre — más la pierna de sucursal, que solo
///   aporta cuando la sucursal existe en la organización, está ACTIVA, la
///   membership está ACTIVA y el rol de la membership está ACTIVO.
/// - Un permiso concedido por membership nunca se promociona a alcance de
///   organización: el scope de organización se resuelve con `branch_id = None`.
/// - Una sucursal inexistente o ajena no produce un error: no aporta permisos y
///   la clave foránea de tenant impide confundirla. No se revela su existencia.
pub trait PermissionAuthority {
    /// Devuelve las grants vigentes del usuario en el scope solicitado.
    fn resolve_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: Option<&EntityId>,
    ) -> impl Future<Output = Result<BTreeSet<PermissionCode>, IdentityError>> + Send;
    /// Variante de recuperación: igual que [`Self::resolve_permissions`] con
    /// `branch_id = Some(b)`, pero la pierna de sucursal no exige que la
    /// sucursal esté ACTIVA.
    ///
    /// Debe destinarse solo a flujos explícitamente autorizados sobre recursos
    /// ya abiertos; no es un bypass global ni desactiva las demás condiciones.
    fn resolve_permissions_for_recovery(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<BTreeSet<PermissionCode>, IdentityError>> + Send;
}

/// Coordinación de autenticación local sobre una única fuente DB-authoritative.
///
/// Identidad y permisos comparten la misma fuente para que la decisión no mezcle
/// orígenes con estados distintos. Para mutaciones críticas el llamador debe
/// construir este servicio sobre la conexión de la unidad de trabajo activa.
pub struct LocalPrincipalService<S> {
    source: S,
}
impl<S: LocalIdentityRepository + PermissionAuthority> LocalPrincipalService<S> {
    /// Ensambla la fuente explícita; no existe implementación allow-all.
    pub fn new(source: S) -> Self {
        Self { source }
    }
    /// Construye un principal para un scope normal y falla cerrado en cualquier duda.
    ///
    /// La organización del principal procede siempre de la sesión y del usuario,
    /// nunca de la petición.
    pub async fn authenticate(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
    ) -> Result<VerifiedPrincipal, AuditAttributionError> {
        self.authenticate_in(hash, now, branch_id, ScopeMode::Normal)
            .await
    }
    /// Construye un principal de recuperación para una sucursal concreta.
    ///
    /// La pierna de sucursal no exige sucursal ACTIVA; el resto de condiciones
    /// (usuario, organización, membership, rol) se mantiene. El principal
    /// resultante solo autoriza scopes en modo recuperación.
    pub async fn authenticate_recovery(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: &EntityId,
    ) -> Result<VerifiedPrincipal, AuditAttributionError> {
        self.authenticate_in(hash, now, Some(branch_id), ScopeMode::Recovery)
            .await
    }
    async fn authenticate_in(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
        mode: ScopeMode,
    ) -> Result<VerifiedPrincipal, AuditAttributionError> {
        if mode == ScopeMode::Recovery && branch_id.is_none() {
            return Err(AuditAttributionError::ScopeDenied);
        }
        let session = self
            .source
            .find_session_by_hash(hash)
            .await
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?
            .ok_or(AuditAttributionError::Unauthenticated)?;
        let user = self
            .source
            .find_user(session.organization_id(), session.user_id())
            .await
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?
            .ok_or(AuditAttributionError::Unauthenticated)?;
        session
            .ensure_owned_by(&user)
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?;
        session
            .validate_for(user.id(), user.organization_id(), now)
            .map_err(|error| match error {
                IdentityError::SessionNotActive => AuditAttributionError::Unauthenticated,
                _ => AuditAttributionError::UnattributablePrincipal,
            })?;
        if !user.is_active() {
            return Err(AuditAttributionError::Unauthenticated);
        }
        let permissions = match (mode, branch_id) {
            (ScopeMode::Recovery, Some(branch)) => {
                self.source
                    .resolve_permissions_for_recovery(user.id(), user.organization_id(), branch)
                    .await
            }
            _ => {
                self.source
                    .resolve_permissions(user.id(), user.organization_id(), branch_id)
                    .await
            }
        }
        .map_err(|_| AuditAttributionError::ScopeDenied)?;
        let scope = match mode {
            ScopeMode::Recovery => {
                let branch = branch_id.ok_or(AuditAttributionError::ScopeDenied)?;
                PrincipalScope::recovery(user.organization_id().clone(), branch.clone())
            }
            ScopeMode::Normal => {
                PrincipalScope::new(user.organization_id().clone(), branch_id.cloned())
            }
        };
        Ok(VerifiedPrincipal::assemble(
            user.id().clone(),
            session.id().clone(),
            scope,
            permissions,
        ))
    }
}
