//! Casos de uso de usuarios de la organización.
//!
//! # Reglas
//!
//! - La organización procede del principal verificado.
//! - Crear un usuario exige `user.create`; modificarlo, `user.update`; asignarle
//!   roles de organización, `role.assign`.
//! - Un administrador no puede modificar sus propios roles ni desactivar su
//!   propia cuenta: así no puede escalar privilegios ni bloquearse a sí mismo.
//! - La organización no puede quedarse sin ningún administrador efectivo
//!   (usuario ACTIVO con rol ACTIVO que concede `user.update`).
//! - Desactivar una cuenta revoca sus sesiones vigentes.
//!
//! # Fuera de alcance
//!
//! No existe todavía restablecimiento de contraseña administrativo: cualquier
//! mecanismo de recuperación exige un contrato propio y pruebas de seguridad.

use pos_core::{
    audit::AuditOccurredAt,
    identity::{NormalizedEmail, PosUser, PosUserPatch, PosUserStatus, Role, VerifiedPrincipal},
    organization_branch::{ApplicationError, DisplayName, EntityId, IdentityGenerator, Permission},
};
use serde_json::json;

use crate::{
    CasOutcome,
    administration::{
        application::{
            AdministrationApplication, domain_error, finish, identity_error, parse_id,
            parse_user_status, require_permission,
        },
        dto::{MIN_USER_PASSWORD_LEN, NewUserRequest, UserSummary, UserUpdate},
        ports::{AdministrationTransaction, AdministrationUnitOfWork, PasswordHasher},
    },
    audit::{AuditAppender, append_audit},
};

/// Límite máximo de usuarios por página.
pub const MAX_USER_PAGE: u32 = 100;

impl<U> AdministrationApplication<U>
where
    U: AdministrationUnitOfWork,
    U::Transaction: AuditAppender + Send,
{
    /// Lista los usuarios de la organización con sus roles.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `user.read` y
    /// [`ApplicationError::InvalidAdministrationInput`] con una página inválida.
    pub async fn list_users(
        &self,
        principal: &VerifiedPrincipal,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<UserSummary>, ApplicationError> {
        let organization_id = require_permission(principal, Permission::UserRead)?;
        if !(1..=MAX_USER_PAGE).contains(&limit) {
            return Err(ApplicationError::InvalidAdministrationInput);
        }
        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let users = transaction
                .list_users(&organization_id, limit, offset)
                .await?;
            let mut summaries = Vec::with_capacity(users.len());
            for user in users {
                let roles = roles_of(&organization_id, user.id(), &mut transaction).await?;
                summaries.push(UserSummary { user, roles });
            }
            Ok(summaries)
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Devuelve un usuario concreto con sus roles.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `user.read` y
    /// [`ApplicationError::UserNotFound`] si no pertenece a la organización.
    pub async fn get_user(
        &self,
        principal: &VerifiedPrincipal,
        user_id: &str,
    ) -> Result<UserSummary, ApplicationError> {
        let organization_id = require_permission(principal, Permission::UserRead)?;
        let user_id = parse_id(user_id)?;
        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let user = transaction
                .get_user(&organization_id, &user_id)
                .await?
                .ok_or(ApplicationError::UserNotFound)?;
            let roles = roles_of(&organization_id, user.id(), &mut transaction).await?;
            Ok(UserSummary { user, roles })
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Crea un usuario activo con su credencial de contraseña.
    ///
    /// La contraseña se deriva con el puerto criptográfico **antes** de abrir la
    /// transacción, para no retener la reserva de escritura de SQLite durante
    /// Argon2id.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `user.create`,
    /// [`ApplicationError::InvalidAdministrationInput`] si los datos no cumplen el
    /// contrato y [`ApplicationError::UserEmailAlreadyExists`] si el email ya está
    /// ocupado en la organización.
    pub async fn create_user<I, H>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        hasher: &H,
        request: NewUserRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<UserSummary, ApplicationError>
    where
        I: IdentityGenerator,
        H: PasswordHasher,
    {
        let organization_id = require_permission(principal, Permission::UserCreate)?;
        let display_name = DisplayName::new(&request.display_name).map_err(domain_error)?;
        let email = NormalizedEmail::new(&request.email).map_err(identity_error)?;
        if request.password.chars().count() < MIN_USER_PASSWORD_LEN {
            return Err(ApplicationError::InvalidAdministrationInput);
        }
        let user_id = identities.next_id()?;
        let credential_id = identities.next_id()?;
        let credential = hasher
            .prepare(
                credential_id,
                user_id.clone(),
                &request.password,
                request.now,
            )
            .await?;
        let user = PosUser::create(user_id, organization_id.clone(), display_name, email);

        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            if transaction
                .user_email_exists(&organization_id, user.email(), None)
                .await?
            {
                return Err(ApplicationError::UserEmailAlreadyExists);
            }
            transaction.insert_user(&user, occurred_at.as_str()).await?;
            transaction
                .insert_credential(&credential, occurred_at.as_str())
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "user.created",
                "pos_user",
                user.id().clone(),
                json!({ "email": user.email().as_str() }),
                occurred_at,
            )
            .await?;
            Ok(UserSummary {
                user,
                roles: Vec::new(),
            })
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Actualiza nombre, email o estado administrativo con CAS.
    ///
    /// Desactivar una cuenta revoca sus sesiones vigentes en la misma transacción.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `user.update` o al intentar
    /// desactivar la propia cuenta, [`ApplicationError::UserNotFound`] si no
    /// pertenece a la organización, [`ApplicationError::ConcurrentModification`]
    /// si la revisión está obsoleta, [`ApplicationError::UserEmailAlreadyExists`]
    /// si el email ya está ocupado y [`ApplicationError::LastAdministrator`] si la
    /// baja dejaría la organización sin administración.
    pub async fn update_user<I>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        user_id: &str,
        update: UserUpdate,
        occurred_at: &AuditOccurredAt,
    ) -> Result<UserSummary, ApplicationError>
    where
        I: IdentityGenerator,
    {
        let organization_id = require_permission(principal, Permission::UserUpdate)?;
        let user_id = parse_id(user_id)?;
        let status = update
            .status
            .as_deref()
            .map(parse_user_status)
            .transpose()?;
        let display_name = update
            .display_name
            .as_deref()
            .map(DisplayName::new)
            .transpose()
            .map_err(domain_error)?;
        let email = update
            .email
            .as_deref()
            .map(NormalizedEmail::new)
            .transpose()
            .map_err(identity_error)?;
        if status == Some(PosUserStatus::Inactive) && user_id == *principal.user_id() {
            return Err(ApplicationError::Forbidden);
        }

        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let mut user = transaction
                .get_user(&organization_id, &user_id)
                .await?
                .ok_or(ApplicationError::UserNotFound)?;
            if let Some(email) = &email
                && email != user.email()
                && transaction
                    .user_email_exists(&organization_id, email, Some(&user_id))
                    .await?
            {
                return Err(ApplicationError::UserEmailAlreadyExists);
            }
            let before = user.clone();
            let previous_revision = user.revision();
            let deactivating =
                status == Some(PosUserStatus::Inactive) && user.status() == PosUserStatus::Active;
            let changed = user
                .patch(PosUserPatch {
                    expected_version: update.expected_revision,
                    display_name,
                    email,
                    status,
                })
                .map_err(identity_error)?;
            if !changed {
                // Sin cambios materiales: ni revisión nueva ni auditoría.
                let roles = roles_of(&organization_id, user.id(), &mut transaction).await?;
                return Ok(UserSummary { user, roles });
            }
            if deactivating
                && transaction
                    .is_active_administrator(&organization_id, &user_id)
                    .await?
                && transaction
                    .count_active_administrators(&organization_id)
                    .await?
                    <= 1
            {
                return Err(ApplicationError::LastAdministrator);
            }
            if transaction
                .update_user_cas(
                    &organization_id,
                    previous_revision,
                    &user,
                    occurred_at.as_str(),
                )
                .await?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            if deactivating {
                transaction
                    .revoke_user_sessions(&organization_id, &user_id, occurred_at.as_str())
                    .await?;
            }
            append_audit(
                &mut transaction,
                identities,
                principal,
                "user.updated",
                "pos_user",
                user.id().clone(),
                json!({ "changedFields": changed_fields(&before, &user) }),
                occurred_at,
            )
            .await?;
            let roles = roles_of(&organization_id, user.id(), &mut transaction).await?;
            Ok(UserSummary { user, roles })
        }
        .await;
        finish(self, transaction, operation).await
    }

    /// Reemplaza los roles de organización de un usuario.
    ///
    /// El cambio de roles es inmediato en la autorización: cada operación
    /// protegida revalida permisos en SQLite, de modo que una sesión abierta deja
    /// de tener los permisos retirados en la siguiente llamada.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Forbidden`] sin `role.assign` o al intentar
    /// cambiar los roles propios, [`ApplicationError::UserNotFound`] y
    /// [`ApplicationError::RoleNotFound`] si alguna identidad no pertenece a la
    /// organización, y [`ApplicationError::LastAdministrator`] si el cambio dejaría
    /// la organización sin administración.
    pub async fn set_user_roles<I>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        user_id: &str,
        role_ids: &[String],
        occurred_at: &AuditOccurredAt,
    ) -> Result<UserSummary, ApplicationError>
    where
        I: IdentityGenerator,
    {
        let organization_id = require_permission(principal, Permission::RoleAssign)?;
        let user_id = parse_id(user_id)?;
        if user_id == *principal.user_id() {
            return Err(ApplicationError::Forbidden);
        }
        let mut wanted = Vec::with_capacity(role_ids.len());
        for value in role_ids {
            wanted.push(parse_id(value)?);
        }
        wanted.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        wanted.dedup();

        let mut transaction = self.unit_of_work().begin_transaction().await?;
        let operation = async {
            let user = transaction
                .get_user(&organization_id, &user_id)
                .await?
                .ok_or(ApplicationError::UserNotFound)?;
            let mut next = Vec::with_capacity(wanted.len());
            for role_id in &wanted {
                next.push(
                    transaction
                        .get_role(&organization_id, role_id)
                        .await?
                        .ok_or(ApplicationError::RoleNotFound)?,
                );
            }
            let mut current = transaction
                .organization_role_ids(&organization_id, &user_id)
                .await?;
            current.sort_by(|left, right| left.as_str().cmp(right.as_str()));
            current.dedup();
            if current == wanted {
                let roles = roles_of(&organization_id, user.id(), &mut transaction).await?;
                return Ok(UserSummary { user, roles });
            }
            if transaction
                .is_active_administrator(&organization_id, &user_id)
                .await?
                && transaction
                    .count_active_administrators(&organization_id)
                    .await?
                    <= 1
                && !grants_administration(&organization_id, &next, &mut transaction).await?
            {
                return Err(ApplicationError::LastAdministrator);
            }
            transaction
                .replace_organization_roles(
                    &organization_id,
                    &user_id,
                    &wanted,
                    occurred_at.as_str(),
                )
                .await?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "user.roles_replaced",
                "pos_user",
                user.id().clone(),
                json!({ "roles": wanted.iter().map(|id| id.as_str()).collect::<Vec<_>>() }),
                occurred_at,
            )
            .await?;
            let roles = roles_of(&organization_id, user.id(), &mut transaction).await?;
            Ok(UserSummary { user, roles })
        }
        .await;
        finish(self, transaction, operation).await
    }
}

/// Roles de organización del usuario resueltos dentro de la transacción.
async fn roles_of<T>(
    organization_id: &EntityId,
    user_id: &EntityId,
    transaction: &mut T,
) -> Result<Vec<Role>, ApplicationError>
where
    T: AdministrationTransaction + Send,
{
    let mut roles = Vec::new();
    for role_id in transaction
        .organization_role_ids(organization_id, user_id)
        .await?
    {
        if let Some(role) = transaction.get_role(organization_id, &role_id).await? {
            roles.push(role);
        }
    }
    roles.sort_by(|left, right| left.code().as_str().cmp(right.code().as_str()));
    Ok(roles)
}

/// Indica si al menos uno de los roles propuestos concede `user.update`.
async fn grants_administration<T>(
    organization_id: &EntityId,
    roles: &[Role],
    transaction: &mut T,
) -> Result<bool, ApplicationError>
where
    T: AdministrationTransaction + Send,
{
    for role in roles {
        if !role.is_active() {
            continue;
        }
        let codes = transaction
            .role_permission_codes(organization_id, role.id())
            .await?;
        if codes.iter().any(|code| code == "user.update") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Campos materiales que difieren entre dos revisiones del usuario.
fn changed_fields(before: &PosUser, after: &PosUser) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if before.display_name() != after.display_name() {
        fields.push("displayName");
    }
    if before.email() != after.email() {
        fields.push("email");
    }
    if before.status() != after.status() {
        fields.push("status");
    }
    fields
}
