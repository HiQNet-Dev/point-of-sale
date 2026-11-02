//! Casos de uso de membresías de sucursal.
//!
//! La membresía es la única vía por la que un usuario obtiene permisos de alcance
//! sucursal. Sin una membresía ACTIVA con rol ACTIVO, `auth_select_branch`
//! resuelve cero permisos para esa pierna y todos los módulos operativos
//! (caja, ventas, inventario) quedan denegados.
//!
//! # Reglas
//!
//! - Leer membresías exige `branch.membership.read`; crearlas,
//!   `branch.membership.create`; actualizar su estado,
//!   `branch.membership.update`; reemplazar sus roles,
//!   `branch.membership.roles.manage`.
//! - La sucursal debe pertenecer a la organización del principal; una sucursal
//!   ajena se comporta como inexistente.
//! - Un principal con sucursal seleccionada solo puede administrar membresías de
//!   esa sucursal.
//! - Organización, sucursal y usuario de una membresía son inmutables.

use pos_core::{
    audit::AuditOccurredAt,
    identity::{BranchMembership, MembershipPatch, Role, VerifiedPrincipal},
    organization_branch::{ApplicationError, EntityId, IdentityGenerator, Permission},
};
use serde_json::json;

use crate::{
    CasOutcome,
    administration::{
        application::{
            AdministrationApplication, finish, identity_error, parse_id, parse_status,
            require_branch_permission, require_permission,
        },
        dto::{MembershipSummary, MembershipUpdate},
        ports::{AdministrationTransaction, AdministrationUnitOfWork},
    },
    audit::{AuditAppender, append_audit},
};

impl<U> AdministrationApplication<U>
where
    U: AdministrationUnitOfWork,
    U::Transaction: AuditAppender + Send,
{
    /// Lista las membresías de la organización con sus roles.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `branch.membership.read`.
    pub async fn list_memberships(
        &self,
        principal: &VerifiedPrincipal,
    ) -> Result<Vec<MembershipSummary>, ApplicationError> {
        let organization_id = require_permission(principal, Permission::BranchMembershipRead)?;
        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let memberships = transaction.list_memberships(&organization_id).await?;
            let mut summaries = Vec::with_capacity(memberships.len());
            for membership in memberships {
                let roles = roles_of(&organization_id, membership.id(), &mut transaction).await?;
                summaries.push(MembershipSummary { membership, roles });
            }
            Ok(summaries)
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Crea una membresía activa para un usuario en una sucursal.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `branch.membership.create`
    /// (o sin `branch.membership.roles.manage` al asignar roles),
    /// [`ApplicationError::BranchNotFound`] y [`ApplicationError::UserNotFound`]
    /// si alguna identidad no pertenece a la organización, y
    /// [`ApplicationError::Conflict`] si la membresía ya existe.
    pub async fn create_membership<I>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        branch_id: &str,
        user_id: &str,
        role_ids: &[String],
        occurred_at: &AuditOccurredAt,
    ) -> Result<MembershipSummary, ApplicationError>
    where
        I: IdentityGenerator,
    {
        let branch_id = parse_id(branch_id)?;
        let organization_id =
            require_branch_permission(principal, Permission::BranchMembershipCreate, &branch_id)?;
        let user_id = parse_id(user_id)?;
        let wanted = parse_roles(role_ids)?;
        if !wanted.is_empty() {
            require_branch_permission(
                principal,
                Permission::BranchMembershipRolesManage,
                &branch_id,
            )?;
        }
        let membership_id = identities.next_id()?;

        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            transaction
                .get_branch(&organization_id, &branch_id)
                .await?
                .ok_or(ApplicationError::BranchNotFound)?;
            transaction
                .get_user(&organization_id, &user_id)
                .await?
                .ok_or(ApplicationError::UserNotFound)?;
            if transaction
                .find_membership(&organization_id, &branch_id, &user_id)
                .await?
                .is_some()
            {
                return Err(ApplicationError::MembershipAlreadyExists);
            }
            let mut roles = Vec::with_capacity(wanted.len());
            for role_id in &wanted {
                roles.push(
                    transaction
                        .get_role(&organization_id, role_id)
                        .await?
                        .ok_or(ApplicationError::RoleNotFound)?,
                );
            }
            let membership = BranchMembership::create(
                membership_id,
                organization_id.clone(),
                branch_id.clone(),
                user_id.clone(),
            );
            transaction
                .insert_membership(&membership, occurred_at.as_str())
                .await?;
            if !wanted.is_empty() {
                transaction
                    .replace_membership_roles(
                        &organization_id,
                        membership.id(),
                        &wanted,
                        occurred_at.as_str(),
                    )
                    .await?;
            }
            append_audit(
                &mut transaction,
                identities,
                principal,
                "branch.membership_created",
                "branch_membership",
                membership.id().clone(),
                json!({ "branch": branch_id.as_str(), "user": user_id.as_str() }),
                occurred_at,
            )
            .await?;
            roles.sort_by(|left, right| left.code().as_str().cmp(right.code().as_str()));
            Ok(MembershipSummary { membership, roles })
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Actualiza el estado y, opcionalmente, los roles de una membresía con CAS.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `branch.membership.update`
    /// (o sin `branch.membership.roles.manage` al cambiar roles),
    /// [`ApplicationError::MembershipNotFound`] si no pertenece a la organización,
    /// [`ApplicationError::RoleNotFound`] si algún rol no existe y
    /// [`ApplicationError::ConcurrentModification`] si la revisión está obsoleta.
    pub async fn update_membership<I>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        membership_id: &str,
        update: MembershipUpdate,
        occurred_at: &AuditOccurredAt,
    ) -> Result<MembershipSummary, ApplicationError>
    where
        I: IdentityGenerator,
    {
        let membership_id = parse_id(membership_id)?;
        let organization_id = require_permission(principal, Permission::BranchMembershipUpdate)?;
        let status = update.status.as_deref().map(parse_status).transpose()?;
        let wanted = update.role_ids.as_deref().map(parse_roles).transpose()?;

        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let mut membership = transaction
                .get_membership(&organization_id, &membership_id)
                .await?
                .ok_or(ApplicationError::MembershipNotFound)?;
            require_branch_permission(
                principal,
                Permission::BranchMembershipUpdate,
                membership.branch_id(),
            )?;
            if wanted.is_some() {
                require_branch_permission(
                    principal,
                    Permission::BranchMembershipRolesManage,
                    membership.branch_id(),
                )?;
            }
            let before = membership.clone();
            let previous_revision = membership.revision();
            let mut changed = membership
                .patch(MembershipPatch {
                    expected_revision: update.expected_revision,
                    status,
                })
                .map_err(identity_error)?;
            let mut roles = roles_of(&organization_id, membership.id(), &mut transaction).await?;
            if let Some(wanted) = &wanted {
                let mut current = transaction
                    .membership_role_ids(&organization_id, membership.id())
                    .await?;
                current.sort_by(|left, right| left.as_str().cmp(right.as_str()));
                current.dedup();
                if &current != wanted {
                    // Validar antes de mover la revisión: un rol inexistente no
                    // debe dejar la membresía con una revisión consumida.
                    roles.clear();
                    for role_id in wanted {
                        roles.push(
                            transaction
                                .get_role(&organization_id, role_id)
                                .await?
                                .ok_or(ApplicationError::RoleNotFound)?,
                        );
                    }
                    if !changed {
                        // El estado no cambió, pero los roles sí: la revisión debe
                        // avanzar para que el CAS siga siendo significativo.
                        membership
                            .touch(previous_revision)
                            .map_err(identity_error)?;
                    }
                    changed = true;
                }
            }
            if !changed {
                return Ok(MembershipSummary { membership, roles });
            }
            if transaction
                .update_membership_cas(
                    &organization_id,
                    previous_revision,
                    &membership,
                    occurred_at.as_str(),
                )
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            if let Some(wanted) = &wanted {
                transaction
                    .replace_membership_roles(
                        &organization_id,
                        membership.id(),
                        wanted,
                        occurred_at.as_str(),
                    )
                    .await?;
            }
            append_audit(
                &mut transaction,
                identities,
                principal,
                "branch.membership_updated",
                "branch_membership",
                membership.id().clone(),
                json!({
                    "changedFields": pos_core::identity::membership_changed_fields(&before, &membership),
                    "rolesChanged": wanted.is_some(),
                }),
                occurred_at,
            )
            .await?;
            if wanted.is_some() {
                roles = roles_of(&organization_id, membership.id(), &mut transaction).await?;
            }
            roles.sort_by(|left, right| left.code().as_str().cmp(right.code().as_str()));
            Ok(MembershipSummary { membership, roles })
        }
        .await;
        finish(self, transaction, operation).await
    }
}

/// Normaliza y ordena las identidades de rol de la petición.
fn parse_roles(role_ids: &[String]) -> Result<Vec<EntityId>, ApplicationError> {
    let mut roles = Vec::with_capacity(role_ids.len());
    for value in role_ids {
        roles.push(parse_id(value)?);
    }
    roles.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    roles.dedup();
    Ok(roles)
}

/// Roles de sucursal de la membresía, ordenados por código.
async fn roles_of<T>(
    organization_id: &EntityId,
    membership_id: &EntityId,
    transaction: &mut T,
) -> Result<Vec<Role>, ApplicationError>
where
    T: AdministrationTransaction + Send,
{
    let mut roles = Vec::new();
    for role_id in transaction
        .membership_role_ids(organization_id, membership_id)
        .await?
    {
        if let Some(role) = transaction.get_role(organization_id, &role_id).await? {
            roles.push(role);
        }
    }
    roles.sort_by(|left, right| left.code().as_str().cmp(right.code().as_str()));
    Ok(roles)
}
