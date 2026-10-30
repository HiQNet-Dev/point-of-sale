use crate::audit::AuditActor;
use crate::organization_branch::EntityId;
use std::collections::BTreeSet;
use std::fmt;

use super::*;

/// Código de permiso resuelto desde el catálogo vigente de base de datos.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PermissionCode(String);
impl PermissionCode {
    /// Acepta códigos técnicos ASCII separados por puntos, sin wildcard.
    ///
    /// El catálogo Java (V5–V25) usa letras minúsculas, dígitos, `_` y `-` en
    /// cada segmento (`cash-register.read`, `purchasing.order.write`).
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || value.len() > 64
            || value.split('.').any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || byte == b'_'
                            || byte == b'-'
                    })
            })
        {
            return Err(IdentityError::InvalidPermission);
        }
        Ok(Self(value.to_owned()))
    }
    /// Código para comparar una decisión de autorización.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Catálogo canónico de permisos del backend Java (V5–V25), 59 códigos.
///
/// No concede nada por sí mismo: es la lista que la migración Local debe sembrar
/// y que el dominio usa para validar el mismo lenguaje que SQLite.
pub const PERMISSION_CATALOG: [&str; 59] = [
    "organization.read",
    "organization.update",
    "branch.create",
    "branch.read",
    "branch.update",
    "branch.membership.create",
    "branch.membership.read",
    "branch.membership.roles.manage",
    "branch.membership.update",
    "user.create",
    "user.read",
    "user.update",
    "role.assign",
    "role.create",
    "role.permissions.manage",
    "role.read",
    "role.update",
    "permission.read",
    "audit.read",
    "category.create",
    "category.read",
    "category.update",
    "product.create",
    "product.modifiers.manage",
    "product.read",
    "product.update",
    "modifier.group.create",
    "modifier.group.read",
    "modifier.group.update",
    "modifier.option.create",
    "modifier.option.read",
    "modifier.option.update",
    "cash-register.create",
    "cash-register.read",
    "cash-register.update",
    "cash-shift.close",
    "cash-shift.open",
    "cash-shift.read",
    "sale.cancel",
    "sale.checkout",
    "sale.create",
    "sale.read",
    "sale.update",
    "payment.read",
    "pos.catalog.read",
    "inventory.item.read",
    "inventory.item.write",
    "inventory.recipe.read",
    "inventory.recipe.write",
    "inventory.stock.adjust",
    "inventory.stock.read",
    "purchasing.order.cancel",
    "purchasing.order.issue",
    "purchasing.order.read",
    "purchasing.order.write",
    "purchasing.receipt.read",
    "purchasing.receipt.write",
    "purchasing.supplier.read",
    "purchasing.supplier.write",
];

/// Modo de evaluación de un scope de sucursal.
///
/// Java distingue `hasBranchPermission` de
/// `hasBranchPermissionIgnoringBranchStatus`: la variante de recuperación existe
/// solo para recursos ya abiertos en una sucursal desactivada (por ejemplo,
/// cerrar un turno de caja) y nunca habilita operaciones normales.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeMode {
    /// Operación normal: la pierna de sucursal exige sucursal ACTIVA.
    Normal,
    /// Recuperación autorizada: la pierna de sucursal no exige sucursal ACTIVA.
    Recovery,
}

/// Scope concreto de una operación: organización, sucursal opcional y modo.
///
/// El scope describe la petición; no concede nada por sí mismo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrincipalScope {
    organization_id: EntityId,
    branch_id: Option<EntityId>,
    mode: ScopeMode,
}
impl PrincipalScope {
    /// Construye un scope normal; la organización se resuelve aparte.
    pub fn new(organization_id: EntityId, branch_id: Option<EntityId>) -> Self {
        Self {
            organization_id,
            branch_id,
            mode: ScopeMode::Normal,
        }
    }
    /// Construye un scope de recuperación explícito para una sucursal concreta.
    pub fn recovery(organization_id: EntityId, branch_id: EntityId) -> Self {
        Self {
            organization_id,
            branch_id: Some(branch_id),
            mode: ScopeMode::Recovery,
        }
    }
    /// Organización del scope.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Sucursal del scope; `None` significa alcance de organización.
    pub fn branch_id(&self) -> Option<&EntityId> {
        self.branch_id.as_ref()
    }
    /// Modo de evaluación del scope.
    pub fn mode(&self) -> ScopeMode {
        self.mode
    }
}

/// Fallo de una decisión contextual de autorización.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationError {
    /// El scope solicitado no es el scope del principal autenticado.
    ScopeMismatch,
    /// El permiso no fue concedido para ese scope.
    PermissionDenied,
}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ScopeMismatch => "el scope no corresponde al principal",
            Self::PermissionDenied => "permiso denegado",
        })
    }
}
impl std::error::Error for AuthorizationError {}

/// Principal ya autenticado y scoped. Sus campos no son elegibles desde transporte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPrincipal {
    user_id: EntityId,
    session_id: EntityId,
    scope: PrincipalScope,
    permissions: BTreeSet<PermissionCode>,
}

impl VerifiedPrincipal {
    /// Ensambla un principal ya validado por el servicio de identidad.
    ///
    /// Es `pub(crate)` porque solo la resolución DB-authoritative puede
    /// demostrar que la sesión, el usuario y los permisos son coherentes. Los
    /// módulos de aplicación lo consumen por sus getters.
    pub(crate) fn assemble(
        user_id: EntityId,
        session_id: EntityId,
        scope: PrincipalScope,
        permissions: BTreeSet<PermissionCode>,
    ) -> Self {
        Self {
            user_id,
            session_id,
            scope,
            permissions,
        }
    }
}
impl VerifiedPrincipal {
    /// Usuario atribuido por sesión validada.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
    /// Sesión Local que correlacionará los hechos auditables.
    pub fn session_id(&self) -> &EntityId {
        &self.session_id
    }
    /// Scope exacto para el que se resolvieron los permisos.
    pub fn scope(&self) -> &PrincipalScope {
        &self.scope
    }
    /// Decide contextualmente: organización, sucursal y permiso en una sola llamada.
    ///
    /// Un permiso resuelto para la sucursal A nunca autoriza la sucursal B ni el
    /// alcance de organización, aunque compartan organización.
    pub fn authorize(
        &self,
        permission: &PermissionCode,
        scope: &PrincipalScope,
    ) -> Result<(), AuthorizationError> {
        if &self.scope != scope {
            return Err(AuthorizationError::ScopeMismatch);
        }
        if !self.permissions.contains(permission) {
            return Err(AuthorizationError::PermissionDenied);
        }
        Ok(())
    }
    /// Construye USER auditado sin recibir actor desde el cliente.
    pub fn audit_actor(&self) -> AuditActor {
        AuditActor::user(self.user_id.clone())
    }
}
