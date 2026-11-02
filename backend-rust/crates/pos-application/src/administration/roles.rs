//! Casos de uso de roles y catálogo de permisos.
//!
//! # Reglas
//!
//! - Leer roles exige `role.read`; crearlos, `role.create`; editarlos,
//!   `role.update`; reemplazar sus permisos, `role.permissions.manage`.
//! - El código del rol es inmutable y único dentro de la organización.
//! - Solo se aceptan códigos del catálogo de permisos del sistema: un rol no
//!   inventa permisos nuevos.
//! - La organización no puede quedarse sin administración al retirar
//!   `user.update` del último rol que la concede.

use std::collections::BTreeSet;

use pos_core::{
    audit::AuditOccurredAt,
    identity::{Role, RolePatch, VerifiedPrincipal, role_changed_fields},
    organization_branch::{
        ApplicationError, BusinessCode, DisplayName, EntityId, IdentityGenerator, Permission,
    },
};
use serde_json::json;

use crate::{
    CasOutcome,
    administration::{
        application::{
            AdministrationApplication, domain_error, finish, identity_error, parse_id,
            parse_revision, parse_status, require_permission,
        },
        dto::{RoleDetail, RoleUpdate},
        ports::{AdministrationTransaction, AdministrationUnitOfWork},
    },
    audit::{AuditAppender, append_audit},
};

/// Permiso cuyo retiro puede dejar la organización sin administración.
const ADMINISTRATION_PERMISSION: &str = "user.update";

impl<U> AdministrationApplication<U>
where
    U: AdministrationUnitOfWork,
    U::Transaction: AuditAppender + Send,
{
    /// Lista los roles de la organización ordenados por código.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `role.read`.
    pub async fn list_roles(
        &self,
        principal: &VerifiedPrincipal,
    ) -> Result<Vec<Role>, ApplicationError> {
        let organization_id = require_permission(principal, Permission::RoleRead)?;
        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = transaction.list_roles(&organization_id).await;
        finish(self, transaction, operation).await
    }

    /// Devuelve un rol con su conjunto de permisos.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `role.read` y
    /// [`ApplicationError::RoleNotFound`] si no pertenece a la organización.
    pub async fn get_role(
        &self,
        principal: &VerifiedPrincipal,
        role_id: &str,
    ) -> Result<RoleDetail, ApplicationError> {
        let organization_id = require_permission(principal, Permission::RoleRead)?;
        let role_id = parse_id(role_id)?;
        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let role = transaction
                .get_role(&organization_id, &role_id)
                .await?
                .ok_or(ApplicationError::RoleNotFound)?;
            let permission_codes =
                permission_set(&organization_id, role.id(), &mut transaction).await?;
            Ok(RoleDetail {
                role,
                permission_codes,
            })
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Crea un rol con un conjunto inicial de permisos.
    ///
    /// Asignar permisos en el alta exige también `role.permissions.manage`: crear
    /// un rol vacío y rellenarlo después no debe ser una vía para saltarse el
    /// permiso de concesión.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `role.create` (o sin
    /// `role.permissions.manage` cuando se conceden permisos),
    /// [`ApplicationError::InvalidAdministrationInput`] si el código, el nombre o
    /// algún permiso no son válidos y
    /// [`ApplicationError::RoleCodeAlreadyExists`] si el código está ocupado.
    pub async fn create_role<I>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        code: &str,
        name: &str,
        permission_codes: &[String],
        occurred_at: &AuditOccurredAt,
    ) -> Result<RoleDetail, ApplicationError>
    where
        I: IdentityGenerator,
    {
        let organization_id = require_permission(principal, Permission::RoleCreate)?;
        let code = BusinessCode::new(code).map_err(domain_error)?;
        let name = DisplayName::new(name).map_err(domain_error)?;
        let codes = normalize_codes(permission_codes)?;
        if !codes.is_empty() {
            require_permission(principal, Permission::RolePermissionsManage)?;
        }
        let role_id = identities.next_id()?;
        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            if transaction
                .role_code_exists(&organization_id, &code)
                .await?
            {
                return Err(ApplicationError::RoleCodeAlreadyExists);
            }
            validate_codes(&codes, &mut transaction).await?;
            let role = Role::create(role_id, organization_id.clone(), code, name);
            transaction.insert_role(&role, occurred_at.as_str()).await?;
            if !codes.is_empty() {
                transaction
                    .replace_role_permissions(
                        &organization_id,
                        role.id(),
                        &codes,
                        occurred_at.as_str(),
                    )
                    .await?;
            }
            append_audit(
                &mut transaction,
                identities,
                principal,
                "role.created",
                "role",
                role.id().clone(),
                json!({ "code": role.code().as_str(), "permissions": codes }),
                occurred_at,
            )
            .await?;
            Ok(RoleDetail {
                role,
                permission_codes: codes.iter().cloned().collect(),
            })
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Actualiza nombre o estado de un rol con CAS.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `role.update`,
    /// [`ApplicationError::RoleNotFound`] si no pertenece a la organización y
    /// [`ApplicationError::ConcurrentModification`] si la revisión está obsoleta.
    pub async fn update_role<I>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        role_id: &str,
        update: RoleUpdate,
        occurred_at: &AuditOccurredAt,
    ) -> Result<Role, ApplicationError>
    where
        I: IdentityGenerator,
    {
        let organization_id = require_permission(principal, Permission::RoleUpdate)?;
        let role_id = parse_id(role_id)?;
        let name = update
            .name
            .as_deref()
            .map(DisplayName::new)
            .transpose()
            .map_err(domain_error)?;
        let status = update.status.as_deref().map(parse_status).transpose()?;

        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let mut role = transaction
                .get_role(&organization_id, &role_id)
                .await?
                .ok_or(ApplicationError::RoleNotFound)?;
            let before = role.clone();
            let previous_revision = role.revision();
            let changed = role
                .patch(RolePatch {
                    expected_revision: update.expected_revision,
                    name,
                    status,
                })
                .map_err(identity_error)?;
            if !changed {
                return Ok(role);
            }
            if transaction
                .update_role_cas(
                    &organization_id,
                    previous_revision,
                    &role,
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
                "role.updated",
                "role",
                role.id().clone(),
                json!({ "changedFields": role_changed_fields(&before, &role) }),
                occurred_at,
            )
            .await?;
            Ok(role)
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Reemplaza el conjunto de permisos de un rol con CAS.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `role.permissions.manage`,
    /// [`ApplicationError::RoleNotFound`] si no pertenece a la organización,
    /// [`ApplicationError::InvalidAdministrationInput`] si algún código no existe
    /// en el catálogo y [`ApplicationError::LastAdministrator`] si el retiro
    /// dejaría la organización sin administración.
    pub async fn set_role_permissions<I>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        role_id: &str,
        expected_revision: &str,
        permission_codes: &[String],
        occurred_at: &AuditOccurredAt,
    ) -> Result<RoleDetail, ApplicationError>
    where
        I: IdentityGenerator,
    {
        let organization_id = require_permission(principal, Permission::RolePermissionsManage)?;
        let role_id = parse_id(role_id)?;
        let expected_revision = parse_revision(expected_revision)?;
        let codes = normalize_codes(permission_codes)?;

        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let mut role = transaction
                .get_role(&organization_id, &role_id)
                .await?
                .ok_or(ApplicationError::RoleNotFound)?;
            let current = permission_set(&organization_id, role.id(), &mut transaction).await?;
            let next: BTreeSet<String> = codes.iter().cloned().collect();
            if current == next {
                return Ok(RoleDetail {
                    role,
                    permission_codes: next,
                });
            }
            validate_codes(&codes, &mut transaction).await?;
            let had_administration = current.iter().any(|code| code == ADMINISTRATION_PERMISSION);
            let keeps_administration = next.iter().any(|code| code == ADMINISTRATION_PERMISSION);
            if had_administration
                && !keeps_administration
                && transaction
                    .count_active_administrators_excluding_role(&organization_id, role.id())
                    .await?
                    == 0
            {
                return Err(ApplicationError::LastAdministrator);
            }
            role.touch(expected_revision).map_err(identity_error)?;
            if transaction
                .update_role_cas(
                    &organization_id,
                    expected_revision,
                    &role,
                    occurred_at.as_str(),
                )
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            transaction
                .replace_role_permissions(&organization_id, role.id(), &codes, occurred_at.as_str())
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "role.permissions_replaced",
                "role",
                role.id().clone(),
                json!({ "permissions": codes }),
                occurred_at,
            )
            .await?;
            Ok(RoleDetail {
                role,
                permission_codes: next,
            })
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Devuelve el catálogo de permisos del sistema.
    ///
    /// Es información de referencia: no concede nada por sí mismo.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `permission.read`.
    pub async fn list_permissions(
        &self,
        principal: &VerifiedPrincipal,
    ) -> Result<Vec<(String, String)>, ApplicationError> {
        require_permission(principal, Permission::PermissionRead)?;
        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = transaction.list_permissions().await;
        finish(self, transaction, operation).await
    }
}

/// Normaliza y ordena el conjunto solicitado, rechazando duplicados ambiguos.
fn normalize_codes(codes: &[String]) -> Result<Vec<String>, ApplicationError> {
    let mut unique = BTreeSet::new();
    for code in codes {
        let trimmed = code.trim();
        if trimmed.is_empty() {
            return Err(ApplicationError::InvalidAdministrationInput);
        }
        unique.insert(trimmed.to_owned());
    }
    Ok(unique.into_iter().collect())
}

/// Comprueba que todos los códigos existen en el catálogo del sistema.
async fn validate_codes<T>(codes: &[String], transaction: &mut T) -> Result<(), ApplicationError>
where
    T: AdministrationTransaction + Send,
{
    for code in codes {
        if !transaction.permission_exists(code).await? {
            return Err(ApplicationError::InvalidAdministrationInput);
        }
    }
    Ok(())
}

/// Conjunto ordenado de permisos persistidos del rol.
async fn permission_set<T>(
    organization_id: &EntityId,
    role_id: &EntityId,
    transaction: &mut T,
) -> Result<BTreeSet<String>, ApplicationError>
where
    T: AdministrationTransaction + Send,
{
    let codes: BTreeSet<String> = transaction
        .role_permission_codes(organization_id, role_id)
        .await?
        .into_iter()
        .collect();
    Ok(codes)
}
