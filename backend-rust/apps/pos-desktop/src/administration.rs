//! Administración de identidad sobre la sesión autenticada.
//!
//! Cada comando revalida el principal en SQLite y delega en
//! `pos-application::administration`. El cliente no aporta organización, actor ni
//! permisos: solo datos administrativos y la revisión que leyó.
//!
//! # Custodia de credenciales
//!
//! La contraseña inicial de un usuario nuevo llega una sola vez, se deriva con
//! Argon2id en un hilo bloqueante y se persiste como hash. El renderer nunca
//! recibe material criptográfico ni puede leer la contraseña de un usuario
//! existente.
//!
//! # Continuidad
//!
//! Un administrador no puede modificar sus propios roles ni desactivar su cuenta,
//! y ninguna operación puede dejar la organización sin administración. Esas
//! reglas viven en la capa de aplicación, no en la interfaz.

use pos_application::administration::{
    AdministrationApplication, MembershipSummary, MembershipUpdate, NewUserRequest, RoleDetail,
    RoleUpdate, UserSummary, UserUpdate,
};
use pos_core::identity::Role;
use pos_storage_sqlite::{
    SqliteAdministrationApplication, SqliteUnitOfWork, administration::LocalPasswordHasher,
};

use crate::{
    auth::SecureIdentities,
    composition::LocalState,
    dto::{
        CreateMembershipRequestDto, IpcError, MembershipDto, NewRoleRequestDto, NewUserRequestDto,
        PermissionDto, RoleDto, RoleRefDto, SetRolePermissionsRequestDto, SetUserRolesRequestDto,
        UpdateMembershipRequestDto, UpdateRoleRequestDto, UpdateUserRequestDto, UserDto,
    },
};

impl LocalState {
    /// Lista los usuarios de la organización con sus roles.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] sin sesión y [`IpcError::Forbidden`]
    /// sin `user.read`.
    pub async fn user_list(&self) -> Result<Vec<UserDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let users = self
            .identity_administration()?
            .list_users(&principal, 100, 0)
            .await?;
        Ok(users.iter().map(UserDto::from).collect())
    }

    /// Devuelve un usuario concreto con sus roles.
    ///
    /// # Errors
    /// Devuelve [`IpcError::NotFound`] si no pertenece a la organización y
    /// [`IpcError::Forbidden`] sin `user.read`.
    pub async fn user_get(&self, user_id: &str) -> Result<UserDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        Ok(UserDto::from(
            &self
                .identity_administration()?
                .get_user(&principal, user_id)
                .await?,
        ))
    }

    /// Crea un usuario activo con su contraseña inicial.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `user.create`,
    /// [`IpcError::ValidationError`] si los datos no cumplen el contrato y
    /// [`IpcError::Conflict`] si el email ya está ocupado.
    pub async fn user_create(&self, request: NewUserRequestDto) -> Result<UserDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let product = self.product_installation()?;
        let (now, occurred_at) = crate::auth::clock(&product.pool).await?;
        let mut identities = SecureIdentities;
        let created = self
            .identity_administration()?
            .create_user(
                &principal,
                &mut identities,
                &LocalPasswordHasher::lab_v1(),
                NewUserRequest {
                    display_name: request.display_name,
                    email: request.email,
                    password: request.password,
                    now,
                },
                &occurred_at,
            )
            .await?;
        Ok(UserDto::from(&created))
    }

    /// Actualiza nombre, email o estado de un usuario.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `user.update` o al intentar
    /// desactivar la propia cuenta, [`IpcError::NotFound`] si no pertenece a la
    /// organización, [`IpcError::Conflict`] con una revisión obsoleta, un email
    /// ocupado o una baja que dejaría la organización sin administración.
    pub async fn user_update(&self, request: UpdateUserRequestDto) -> Result<UserDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let updated = self
            .identity_administration()?
            .update_user(
                &principal,
                &mut identities,
                &request.user_id,
                UserUpdate {
                    expected_revision: revision(&request.expected_revision)?,
                    display_name: request.display_name,
                    email: request.email,
                    status: request.status,
                },
                &occurred_at,
            )
            .await?;
        Ok(UserDto::from(&updated))
    }

    /// Reemplaza los roles de organización de un usuario.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `role.assign` o al intentar cambiar
    /// los roles propios, [`IpcError::NotFound`] si el usuario o algún rol no
    /// pertenece a la organización y [`IpcError::Conflict`] si el cambio dejaría
    /// la organización sin administración.
    pub async fn user_set_roles(
        &self,
        request: SetUserRolesRequestDto,
    ) -> Result<UserDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let updated = self
            .identity_administration()?
            .set_user_roles(
                &principal,
                &mut identities,
                &request.user_id,
                &request.role_ids,
                &occurred_at,
            )
            .await?;
        Ok(UserDto::from(&updated))
    }

    /// Lista los roles de la organización.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `role.read`.
    pub async fn role_list(&self) -> Result<Vec<RoleDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let roles = self
            .identity_administration()?
            .list_roles(&principal)
            .await?;
        Ok(roles.iter().map(RoleDto::from_role).collect())
    }

    /// Devuelve un rol con sus permisos.
    ///
    /// # Errors
    /// Devuelve [`IpcError::NotFound`] si no pertenece a la organización y
    /// [`IpcError::Forbidden`] sin `role.read`.
    pub async fn role_get(&self, role_id: &str) -> Result<RoleDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        Ok(RoleDto::from(
            &self
                .identity_administration()?
                .get_role(&principal, role_id)
                .await?,
        ))
    }

    /// Crea un rol con un conjunto inicial de permisos.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `role.create` o, al conceder
    /// permisos, sin `role.permissions.manage`; [`IpcError::ValidationError`] si
    /// el código, el nombre o un permiso no son válidos; [`IpcError::Conflict`]
    /// si el código está ocupado.
    pub async fn role_create(&self, request: NewRoleRequestDto) -> Result<RoleDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let created = self
            .identity_administration()?
            .create_role(
                &principal,
                &mut identities,
                &request.code,
                &request.name,
                &request.permission_codes,
                &occurred_at,
            )
            .await?;
        Ok(RoleDto::from(&created))
    }

    /// Actualiza nombre o estado de un rol.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `role.update`, [`IpcError::NotFound`]
    /// si no pertenece a la organización y [`IpcError::Conflict`] con una revisión
    /// obsoleta.
    pub async fn role_update(&self, request: UpdateRoleRequestDto) -> Result<RoleDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let role = self
            .identity_administration()?
            .update_role(
                &principal,
                &mut identities,
                &request.role_id,
                RoleUpdate {
                    expected_revision: revision(&request.expected_revision)?,
                    name: request.name,
                    status: request.status,
                },
                &occurred_at,
            )
            .await?;
        Ok(RoleDto::from_role(&role))
    }

    /// Reemplaza el conjunto de permisos de un rol.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `role.permissions.manage`,
    /// [`IpcError::NotFound`] si el rol no pertenece a la organización,
    /// [`IpcError::ValidationError`] si algún permiso no existe y
    /// [`IpcError::Conflict`] con una revisión obsoleta o si el cambio dejaría la
    /// organización sin administración.
    pub async fn role_set_permissions(
        &self,
        request: SetRolePermissionsRequestDto,
    ) -> Result<RoleDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let detail = self
            .identity_administration()?
            .set_role_permissions(
                &principal,
                &mut identities,
                &request.role_id,
                &request.expected_revision,
                &request.permission_codes,
                &occurred_at,
            )
            .await?;
        Ok(RoleDto::from(&detail))
    }

    /// Devuelve el catálogo de permisos del sistema.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `permission.read`.
    pub async fn permission_list(&self) -> Result<Vec<PermissionDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let permissions = self
            .identity_administration()?
            .list_permissions(&principal)
            .await?;
        Ok(permissions
            .into_iter()
            .map(|(code, description)| PermissionDto { code, description })
            .collect())
    }

    /// Lista las membresías de sucursal de la organización.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `branch.membership.read`.
    pub async fn membership_list(&self) -> Result<Vec<MembershipDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let memberships = self
            .identity_administration()?
            .list_memberships(&principal)
            .await?;
        Ok(memberships.iter().map(MembershipDto::from).collect())
    }

    /// Crea una membresía de sucursal con sus roles.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `branch.membership.create` o, al
    /// asignar roles, sin `branch.membership.roles.manage`;
    /// [`IpcError::NotFound`] si la sucursal o el usuario no pertenecen a la
    /// organización; [`IpcError::Conflict`] si la membresía ya existe.
    pub async fn membership_create(
        &self,
        request: CreateMembershipRequestDto,
    ) -> Result<MembershipDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let created = self
            .identity_administration()?
            .create_membership(
                &principal,
                &mut identities,
                &request.branch_id,
                &request.user_id,
                &request.role_ids,
                &occurred_at,
            )
            .await?;
        Ok(MembershipDto::from(&created))
    }

    /// Actualiza el estado y, opcionalmente, los roles de una membresía.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `branch.membership.update` o, al
    /// cambiar roles, sin `branch.membership.roles.manage`;
    /// [`IpcError::NotFound`] si la membresía o algún rol no pertenecen a la
    /// organización; [`IpcError::Conflict`] con una revisión obsoleta.
    pub async fn membership_update(
        &self,
        request: UpdateMembershipRequestDto,
    ) -> Result<MembershipDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let updated = self
            .identity_administration()?
            .update_membership(
                &principal,
                &mut identities,
                &request.membership_id,
                MembershipUpdate {
                    expected_revision: revision(&request.expected_revision)?,
                    status: request.status,
                    role_ids: request.role_ids,
                },
                &occurred_at,
            )
            .await?;
        Ok(MembershipDto::from(&updated))
    }

    fn identity_administration(&self) -> Result<SqliteAdministrationApplication, IpcError> {
        let product = self.product_installation()?;
        Ok(AdministrationApplication::new(SqliteUnitOfWork::new(
            product.pool.clone(),
        )))
    }
}

/// Parser estricto de la revisión enviada como texto decimal.
fn revision(value: &str) -> Result<u64, IpcError> {
    value.parse::<u64>().map_err(|_| IpcError::ValidationError)
}

impl From<&UserSummary> for UserDto {
    fn from(summary: &UserSummary) -> Self {
        Self {
            id: summary.user.id().as_str().into(),
            display_name: summary.user.display_name().as_str().into(),
            email: summary.user.email().as_str().into(),
            status: match summary.user.status() {
                pos_core::identity::PosUserStatus::Active => "ACTIVE",
                pos_core::identity::PosUserStatus::Inactive => "INACTIVE",
            }
            .into(),
            revision: summary.user.revision().to_string(),
            roles: summary.roles.iter().map(RoleRefDto::from).collect(),
        }
    }
}

impl From<&Role> for RoleRefDto {
    fn from(role: &Role) -> Self {
        Self {
            id: role.id().as_str().into(),
            code: role.code().as_str().into(),
            name: role.name().as_str().into(),
            status: role.status().as_str().into(),
            revision: role.revision().to_string(),
        }
    }
}

impl RoleDto {
    /// Rol sin el detalle de permisos, para listados.
    fn from_role(role: &Role) -> Self {
        Self {
            id: role.id().as_str().into(),
            code: role.code().as_str().into(),
            name: role.name().as_str().into(),
            status: role.status().as_str().into(),
            revision: role.revision().to_string(),
            permission_codes: Vec::new(),
        }
    }
}

impl From<&RoleDetail> for RoleDto {
    fn from(detail: &RoleDetail) -> Self {
        Self {
            id: detail.role.id().as_str().into(),
            code: detail.role.code().as_str().into(),
            name: detail.role.name().as_str().into(),
            status: detail.role.status().as_str().into(),
            revision: detail.role.revision().to_string(),
            permission_codes: detail.permission_codes.iter().cloned().collect(),
        }
    }
}

impl From<&MembershipSummary> for MembershipDto {
    fn from(summary: &MembershipSummary) -> Self {
        Self {
            id: summary.membership.id().as_str().into(),
            branch_id: summary.membership.branch_id().as_str().into(),
            user_id: summary.membership.user_id().as_str().into(),
            status: summary.membership.status().as_str().into(),
            revision: summary.membership.revision().to_string(),
            roles: summary.roles.iter().map(RoleRefDto::from).collect(),
        }
    }
}
