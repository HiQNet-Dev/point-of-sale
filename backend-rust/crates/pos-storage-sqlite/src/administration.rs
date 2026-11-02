//! Adaptador SQLite de la administración de identidad.
//!
//! Implementa los puertos de `pos-application::administration` sobre la misma
//! frontera transaccional que el resto de módulos: una conexión con
//! `BEGIN IMMEDIATE`, la mutación, el CAS, la escritura de auditoría y un único
//! commit. Si la auditoría falla, la mutación se revierte con ella.
//!
//! # Ownership
//!
//! Todas las sentencias incluyen `organization_id`. Una fila de otra organización
//! se comporta como inexistente y las claves foráneas compuestas del esquema
//! (`0001`–`0017`) respaldan la barrera.
//!
//! # Credenciales
//!
//! Solo se inserta material ya derivado por el puerto criptográfico. El adaptador
//! nunca hashea, nunca compara contraseñas y nunca devuelve el hash fuera de la
//! transacción.

use pos_application::administration::{
    AdministrationApplication, AdministrationTransaction, AdministrationUnitOfWork, PasswordHasher,
};
use pos_core::{
    identity::{
        BranchMembership, CredentialAlgorithm, CredentialKind, CredentialState, LocalCredential,
        NormalizedEmail, PosUser, PosUserStatus, Role,
    },
    organization_branch::{ApplicationError, Branch, BusinessCode, DisplayName, EntityId, Status},
};

use crate::prelude::transaction_error;
use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork};

/// Alias de composición: los casos de uso viven en `pos-application`.
pub type SqliteAdministrationApplication = AdministrationApplication<SqliteUnitOfWork>;

impl AdministrationUnitOfWork for SqliteUnitOfWork {
    type Transaction = SqliteTransaction;

    async fn begin_transaction(&self) -> Result<SqliteTransaction, ApplicationError> {
        SqliteUnitOfWork::begin(self)
            .await
            .map_err(transaction_error)
    }

    async fn commit_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.commit().await.map_err(transaction_error)
    }

    async fn rollback_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.rollback().await.map_err(transaction_error)
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    organization_id: String,
    display_name: String,
    email: String,
    status: String,
    version: i64,
}

impl UserRow {
    fn into_domain(self) -> Result<PosUser, ApplicationError> {
        let revision = u64::try_from(self.version).map_err(|_| ApplicationError::Storage)?;
        Ok(PosUser::restore(
            EntityId::parse(&self.id).map_err(|_| ApplicationError::Storage)?,
            EntityId::parse(&self.organization_id).map_err(|_| ApplicationError::Storage)?,
            DisplayName::new(&self.display_name).map_err(|_| ApplicationError::Storage)?,
            NormalizedEmail::new(&self.email).map_err(|_| ApplicationError::Storage)?,
            status_of(&self.status)?,
            revision,
        ))
    }
}

#[derive(sqlx::FromRow)]
struct RoleRow {
    id: String,
    organization_id: String,
    code: String,
    name: String,
    status: String,
    version: i64,
}

impl RoleRow {
    fn into_domain(self) -> Result<Role, ApplicationError> {
        let revision = u64::try_from(self.version).map_err(|_| ApplicationError::Storage)?;
        Ok(Role::restore(
            EntityId::parse(&self.id).map_err(|_| ApplicationError::Storage)?,
            EntityId::parse(&self.organization_id).map_err(|_| ApplicationError::Storage)?,
            BusinessCode::new(&self.code).map_err(|_| ApplicationError::Storage)?,
            DisplayName::new(&self.name).map_err(|_| ApplicationError::Storage)?,
            Status::parse(&self.status).map_err(|_| ApplicationError::Storage)?,
            revision,
        ))
    }
}

#[derive(sqlx::FromRow)]
struct MembershipRow {
    id: String,
    organization_id: String,
    branch_id: String,
    user_id: String,
    status: String,
    version: i64,
}

impl MembershipRow {
    fn into_domain(self) -> Result<BranchMembership, ApplicationError> {
        let revision = u64::try_from(self.version).map_err(|_| ApplicationError::Storage)?;
        Ok(BranchMembership::restore(
            EntityId::parse(&self.id).map_err(|_| ApplicationError::Storage)?,
            EntityId::parse(&self.organization_id).map_err(|_| ApplicationError::Storage)?,
            EntityId::parse(&self.branch_id).map_err(|_| ApplicationError::Storage)?,
            EntityId::parse(&self.user_id).map_err(|_| ApplicationError::Storage)?,
            Status::parse(&self.status).map_err(|_| ApplicationError::Storage)?,
            revision,
        ))
    }
}

/// SQL que identifica a un administrador efectivo.
///
/// Un administrador efectivo es un usuario ACTIVO de una organización ACTIVA con
/// un rol ACTIVO que concede `user.update`. Es la única definición que usan los
/// casos de uso para proteger la continuidad de la instalación.
const ACTIVE_ADMINISTRATOR_SQL: &str = "SELECT DISTINCT u.id FROM pos_user u \
     JOIN organization o ON o.id = u.organization_id \
     JOIN user_organization_role ur ON ur.user_id = u.id AND ur.organization_id = u.organization_id \
     JOIN role r ON r.id = ur.role_id AND r.organization_id = ur.organization_id \
     JOIN role_permission rp ON rp.role_id = r.id \
     WHERE u.organization_id = ? AND u.status = 'ACTIVE' AND o.status = 'ACTIVE' \
       AND r.status = 'ACTIVE' AND rp.permission_code = 'user.update'";

/// Variante acotada a un usuario concreto.
const ACTIVE_ADMINISTRATOR_OF_USER_SQL: &str = "SELECT DISTINCT u.id FROM pos_user u \
     JOIN organization o ON o.id = u.organization_id \
     JOIN user_organization_role ur ON ur.user_id = u.id AND ur.organization_id = u.organization_id \
     JOIN role r ON r.id = ur.role_id AND r.organization_id = ur.organization_id \
     JOIN role_permission rp ON rp.role_id = r.id \
     WHERE u.organization_id = ? AND u.id = ? AND u.status = 'ACTIVE' AND o.status = 'ACTIVE' \
       AND r.status = 'ACTIVE' AND rp.permission_code = 'user.update'";

/// Variante que ignora un rol concreto, para editar sus permisos sin dejar la
/// organización sin administración.
const ACTIVE_ADMINISTRATOR_EXCLUDING_ROLE_SQL: &str = "SELECT DISTINCT u.id FROM pos_user u \
     JOIN organization o ON o.id = u.organization_id \
     JOIN user_organization_role ur ON ur.user_id = u.id AND ur.organization_id = u.organization_id \
     JOIN role r ON r.id = ur.role_id AND r.organization_id = ur.organization_id \
     JOIN role_permission rp ON rp.role_id = r.id \
     WHERE u.organization_id = ? AND r.id <> ? AND u.status = 'ACTIVE' AND o.status = 'ACTIVE' \
       AND r.status = 'ACTIVE' AND rp.permission_code = 'user.update'";

impl AdministrationTransaction for SqliteTransaction {
    async fn get_user(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Option<PosUser>, ApplicationError> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, organization_id, display_name, email, status, version \
             FROM pos_user WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .fetch_optional(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        row.map(UserRow::into_domain).transpose()
    }

    async fn list_users(
        &mut self,
        organization_id: &EntityId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PosUser>, ApplicationError> {
        let rows = sqlx::query_as::<_, UserRow>(
            "SELECT id, organization_id, display_name, email, status, version \
             FROM pos_user WHERE organization_id = ? ORDER BY email ASC LIMIT ? OFFSET ?",
        )
        .bind(organization_id.as_str())
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter().map(UserRow::into_domain).collect()
    }

    async fn user_email_exists(
        &mut self,
        organization_id: &EntityId,
        email: &NormalizedEmail,
        excluding: Option<&EntityId>,
    ) -> Result<bool, ApplicationError> {
        let found = match excluding {
            Some(excluded) => sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM pos_user WHERE organization_id = ? AND email = ? AND id <> ? LIMIT 1",
            )
            .bind(organization_id.as_str())
            .bind(email.as_str())
            .bind(excluded.as_str())
            .fetch_optional(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?,
            None => sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM pos_user WHERE organization_id = ? AND email = ? LIMIT 1",
            )
            .bind(organization_id.as_str())
            .bind(email.as_str())
            .fetch_optional(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?,
        };
        Ok(found.is_some())
    }

    async fn insert_user(
        &mut self,
        user: &PosUser,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        let revision = i64::try_from(user.revision()).map_err(|_| ApplicationError::Storage)?;
        sqlx::query(
            "INSERT INTO pos_user \
             (id, organization_id, display_name, email, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user.id().as_str())
        .bind(user.organization_id().as_str())
        .bind(user.display_name().as_str())
        .bind(user.email().as_str())
        .bind(status_code(user.status()))
        .bind(revision)
        .bind(occurred_at)
        .bind(occurred_at)
        .execute(self.connection())
        .await
        .map_err(|error| write_error(error, ApplicationError::UserEmailAlreadyExists))?;
        Ok(())
    }

    async fn update_user_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        user: &PosUser,
        occurred_at: &str,
    ) -> Result<CasOutcome, ApplicationError> {
        let previous = i64::try_from(previous_revision).map_err(|_| ApplicationError::Storage)?;
        let next = i64::try_from(user.revision()).map_err(|_| ApplicationError::Storage)?;
        let result = sqlx::query(
            "UPDATE pos_user SET display_name = ?, email = ?, status = ?, version = ?, \
             updated_at = ? WHERE organization_id = ? AND id = ? AND version = ?",
        )
        .bind(user.display_name().as_str())
        .bind(user.email().as_str())
        .bind(status_code(user.status()))
        .bind(next)
        .bind(occurred_at)
        .bind(organization_id.as_str())
        .bind(user.id().as_str())
        .bind(previous)
        .execute(self.connection())
        .await
        .map_err(|error| write_error(error, ApplicationError::UserEmailAlreadyExists))?;
        Ok(cas_outcome(result.rows_affected()))
    }

    async fn insert_credential(
        &mut self,
        credential: &LocalCredential,
        _occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        if credential.state() != CredentialState::Active {
            // Un alta administrativa solo puede crear material utilizable.
            return Err(ApplicationError::Storage);
        }
        let version = i64::try_from(credential.credential_version())
            .map_err(|_| ApplicationError::Storage)?;
        let attempts = i64::from(credential.failed_login_attempts());
        // La organización se deriva del usuario dueño de la credencial; la FK
        // compuesta `(user_id, organization_id)` vuelve a comprobarlo.
        let organization_id = organization_for_credential(self, credential).await?;
        sqlx::query(
            "INSERT INTO local_credential \
             (id, user_id, organization_id, credential_kind, algorithm, secret_hash, params_json, \
              policy_id, state, credential_version, failed_login_attempts, locked_until, \
              created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'ACTIVE', ?, ?, NULL, \
                 strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch'), \
                 strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch'))",
        )
        .bind(credential.id().as_str())
        .bind(credential.user_id().as_str())
        .bind(organization_id)
        .bind(kind_code(credential.kind()))
        .bind(algorithm_code(credential.algorithm()))
        .bind(credential.secret_hash().expose_to_verifier())
        .bind(credential.parameters().canonical_json())
        .bind(credential.policy_id())
        .bind(version)
        .bind(attempts)
        .bind(credential.created_at().get())
        .bind(Some(credential.updated_at().get()))
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(())
    }

    async fn user_has_password(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<bool, ApplicationError> {
        let found = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM local_credential WHERE organization_id = ? AND user_id = ? \
             AND credential_kind = 'PASSWORD' LIMIT 1",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .fetch_optional(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(found.is_some())
    }

    async fn revoke_user_sessions(
        &mut self,
        _organization_id: &EntityId,
        user_id: &EntityId,
        occurred_at: &str,
    ) -> Result<u64, ApplicationError> {
        let result = sqlx::query(
            "UPDATE local_session SET state = 'REVOKED', revoked_at = ? \
             WHERE user_id = ? AND state = 'ACTIVE'",
        )
        .bind(occurred_at)
        .bind(user_id.as_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(result.rows_affected())
    }

    async fn count_active_administrators(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<u64, ApplicationError> {
        let found = sqlx::query_scalar::<_, String>(ACTIVE_ADMINISTRATOR_SQL)
            .bind(organization_id.as_str())
            .fetch_all(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        Ok(found.len() as u64)
    }

    async fn is_active_administrator(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<bool, ApplicationError> {
        let found = sqlx::query_scalar::<_, String>(ACTIVE_ADMINISTRATOR_OF_USER_SQL)
            .bind(organization_id.as_str())
            .bind(user_id.as_str())
            .fetch_optional(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        Ok(found.is_some())
    }

    async fn count_active_administrators_excluding_role(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
    ) -> Result<u64, ApplicationError> {
        let found = sqlx::query_scalar::<_, String>(ACTIVE_ADMINISTRATOR_EXCLUDING_ROLE_SQL)
            .bind(organization_id.as_str())
            .bind(role_id.as_str())
            .fetch_all(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        Ok(found.len() as u64)
    }

    async fn get_role(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
    ) -> Result<Option<Role>, ApplicationError> {
        let row = sqlx::query_as::<_, RoleRow>(
            "SELECT id, organization_id, code, name, status, version FROM role \
             WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(role_id.as_str())
        .fetch_optional(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        row.map(RoleRow::into_domain).transpose()
    }

    async fn list_roles(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Vec<Role>, ApplicationError> {
        let rows = sqlx::query_as::<_, RoleRow>(
            "SELECT id, organization_id, code, name, status, version FROM role \
             WHERE organization_id = ? ORDER BY code ASC",
        )
        .bind(organization_id.as_str())
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter().map(RoleRow::into_domain).collect()
    }

    async fn role_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &BusinessCode,
    ) -> Result<bool, ApplicationError> {
        let found = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM role WHERE organization_id = ? AND code = ? LIMIT 1",
        )
        .bind(organization_id.as_str())
        .bind(code.as_str())
        .fetch_optional(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(found.is_some())
    }

    async fn insert_role(
        &mut self,
        role: &Role,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        let revision = i64::try_from(role.revision()).map_err(|_| ApplicationError::Storage)?;
        sqlx::query(
            "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(role.id().as_str())
        .bind(role.organization_id().as_str())
        .bind(role.code().as_str())
        .bind(role.name().as_str())
        .bind(role.status().as_str())
        .bind(revision)
        .bind(occurred_at)
        .bind(occurred_at)
        .execute(self.connection())
        .await
        .map_err(|error| write_error(error, ApplicationError::RoleCodeAlreadyExists))?;
        Ok(())
    }

    async fn update_role_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        role: &Role,
        occurred_at: &str,
    ) -> Result<CasOutcome, ApplicationError> {
        let previous = i64::try_from(previous_revision).map_err(|_| ApplicationError::Storage)?;
        let next = i64::try_from(role.revision()).map_err(|_| ApplicationError::Storage)?;
        let result = sqlx::query(
            "UPDATE role SET name = ?, status = ?, version = ?, updated_at = ? \
             WHERE organization_id = ? AND id = ? AND version = ?",
        )
        .bind(role.name().as_str())
        .bind(role.status().as_str())
        .bind(next)
        .bind(occurred_at)
        .bind(organization_id.as_str())
        .bind(role.id().as_str())
        .bind(previous)
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(cas_outcome(result.rows_affected()))
    }

    async fn role_permission_codes(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
    ) -> Result<Vec<String>, ApplicationError> {
        sqlx::query_scalar::<_, String>(
            "SELECT rp.permission_code FROM role_permission rp \
             JOIN role r ON r.id = rp.role_id \
             WHERE r.organization_id = ? AND rp.role_id = ? ORDER BY rp.permission_code ASC",
        )
        .bind(organization_id.as_str())
        .bind(role_id.as_str())
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)
    }

    async fn replace_role_permissions(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
        codes: &[String],
        _occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(
            "DELETE FROM role_permission WHERE role_id = ? AND role_id IN \
             (SELECT id FROM role WHERE organization_id = ? AND id = ?)",
        )
        .bind(role_id.as_str())
        .bind(organization_id.as_str())
        .bind(role_id.as_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        for code in codes {
            sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
                .bind(role_id.as_str())
                .bind(code)
                .execute(self.connection())
                .await
                .map_err(|_| ApplicationError::Storage)?;
        }
        Ok(())
    }

    async fn permission_exists(&mut self, code: &str) -> Result<bool, ApplicationError> {
        let found = sqlx::query_scalar::<_, i64>("SELECT 1 FROM permission WHERE code = ? LIMIT 1")
            .bind(code)
            .fetch_optional(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        Ok(found.is_some())
    }

    async fn list_permissions(&mut self) -> Result<Vec<(String, String)>, ApplicationError> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT code, description FROM permission ORDER BY code ASC",
        )
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(rows)
    }

    async fn organization_role_ids(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Vec<EntityId>, ApplicationError> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT role_id FROM user_organization_role \
             WHERE organization_id = ? AND user_id = ? ORDER BY role_id ASC",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter()
            .map(|value| EntityId::parse(&value).map_err(|_| ApplicationError::Storage))
            .collect()
    }

    async fn replace_organization_roles(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
        role_ids: &[EntityId],
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query("DELETE FROM user_organization_role WHERE organization_id = ? AND user_id = ?")
            .bind(organization_id.as_str())
            .bind(user_id.as_str())
            .execute(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        for role_id in role_ids {
            sqlx::query(
                "INSERT INTO user_organization_role (organization_id, user_id, role_id, created_at) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(organization_id.as_str())
            .bind(user_id.as_str())
            .bind(role_id.as_str())
            .bind(occurred_at)
            .execute(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        }
        Ok(())
    }

    async fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Option<Branch>, ApplicationError> {
        SqliteTransaction::get_branch(self, organization_id, branch_id)
            .await
            .map_err(transaction_error)
    }

    async fn list_memberships(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Vec<BranchMembership>, ApplicationError> {
        let rows = sqlx::query_as::<_, MembershipRow>(
            "SELECT id, organization_id, branch_id, user_id, status, version \
             FROM branch_membership WHERE organization_id = ? ORDER BY branch_id, user_id",
        )
        .bind(organization_id.as_str())
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter().map(MembershipRow::into_domain).collect()
    }

    async fn get_membership(
        &mut self,
        organization_id: &EntityId,
        membership_id: &EntityId,
    ) -> Result<Option<BranchMembership>, ApplicationError> {
        let row = sqlx::query_as::<_, MembershipRow>(
            "SELECT id, organization_id, branch_id, user_id, status, version \
             FROM branch_membership WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(membership_id.as_str())
        .fetch_optional(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        row.map(MembershipRow::into_domain).transpose()
    }

    async fn find_membership(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Option<BranchMembership>, ApplicationError> {
        let row = sqlx::query_as::<_, MembershipRow>(
            "SELECT id, organization_id, branch_id, user_id, status, version \
             FROM branch_membership WHERE organization_id = ? AND branch_id = ? AND user_id = ?",
        )
        .bind(organization_id.as_str())
        .bind(branch_id.as_str())
        .bind(user_id.as_str())
        .fetch_optional(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        row.map(MembershipRow::into_domain).transpose()
    }

    async fn insert_membership(
        &mut self,
        membership: &BranchMembership,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        let revision =
            i64::try_from(membership.revision()).map_err(|_| ApplicationError::Storage)?;
        sqlx::query(
            "INSERT INTO branch_membership \
             (id, organization_id, branch_id, user_id, status, version, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(membership.id().as_str())
        .bind(membership.organization_id().as_str())
        .bind(membership.branch_id().as_str())
        .bind(membership.user_id().as_str())
        .bind(membership.status().as_str())
        .bind(revision)
        .bind(occurred_at)
        .bind(occurred_at)
        .execute(self.connection())
        .await
        .map_err(|error| write_error(error, ApplicationError::MembershipAlreadyExists))?;
        Ok(())
    }

    async fn update_membership_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        membership: &BranchMembership,
        occurred_at: &str,
    ) -> Result<CasOutcome, ApplicationError> {
        let previous = i64::try_from(previous_revision).map_err(|_| ApplicationError::Storage)?;
        let next = i64::try_from(membership.revision()).map_err(|_| ApplicationError::Storage)?;
        let result = sqlx::query(
            "UPDATE branch_membership SET status = ?, version = ?, updated_at = ? \
             WHERE organization_id = ? AND id = ? AND version = ?",
        )
        .bind(membership.status().as_str())
        .bind(next)
        .bind(occurred_at)
        .bind(organization_id.as_str())
        .bind(membership.id().as_str())
        .bind(previous)
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(cas_outcome(result.rows_affected()))
    }

    async fn membership_role_ids(
        &mut self,
        organization_id: &EntityId,
        membership_id: &EntityId,
    ) -> Result<Vec<EntityId>, ApplicationError> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT role_id FROM branch_membership_role \
             WHERE organization_id = ? AND membership_id = ? ORDER BY role_id ASC",
        )
        .bind(organization_id.as_str())
        .bind(membership_id.as_str())
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter()
            .map(|value| EntityId::parse(&value).map_err(|_| ApplicationError::Storage))
            .collect()
    }

    async fn replace_membership_roles(
        &mut self,
        organization_id: &EntityId,
        membership_id: &EntityId,
        role_ids: &[EntityId],
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query(
            "DELETE FROM branch_membership_role WHERE organization_id = ? AND membership_id = ?",
        )
        .bind(organization_id.as_str())
        .bind(membership_id.as_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        for role_id in role_ids {
            sqlx::query(
                "INSERT INTO branch_membership_role \
                 (organization_id, membership_id, role_id, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(organization_id.as_str())
            .bind(membership_id.as_str())
            .bind(role_id.as_str())
            .bind(occurred_at)
            .execute(self.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
        }
        Ok(())
    }
}

/// Organización del usuario dueño de la credencial, leída dentro de la UoW.
async fn organization_for_credential(
    transaction: &mut SqliteTransaction,
    credential: &LocalCredential,
) -> Result<String, ApplicationError> {
    let organization: Option<String> =
        sqlx::query_scalar("SELECT organization_id FROM pos_user WHERE id = ?")
            .bind(credential.user_id().as_str())
            .fetch_optional(transaction.connection())
            .await
            .map_err(|_| ApplicationError::Storage)?;
    organization.ok_or(ApplicationError::UserNotFound)
}

fn cas_outcome(rows: u64) -> CasOutcome {
    if rows == 1 {
        CasOutcome::Updated
    } else {
        CasOutcome::Conflict
    }
}

fn write_error(error: sqlx::Error, duplicate: ApplicationError) -> ApplicationError {
    if error
        .as_database_error()
        .is_some_and(|database| database.is_unique_violation())
    {
        duplicate
    } else {
        ApplicationError::Storage
    }
}

fn status_code(status: PosUserStatus) -> &'static str {
    match status {
        PosUserStatus::Active => "ACTIVE",
        PosUserStatus::Inactive => "INACTIVE",
    }
}

fn status_of(value: &str) -> Result<PosUserStatus, ApplicationError> {
    match value {
        "ACTIVE" => Ok(PosUserStatus::Active),
        "INACTIVE" => Ok(PosUserStatus::Inactive),
        _ => Err(ApplicationError::Storage),
    }
}

fn kind_code(kind: CredentialKind) -> &'static str {
    match kind {
        CredentialKind::Password => "PASSWORD",
        CredentialKind::Pin => "PIN",
    }
}

fn algorithm_code(algorithm: CredentialAlgorithm) -> &'static str {
    match algorithm {
        CredentialAlgorithm::LegacyBcrypt => "LEGACY_BCRYPT",
        CredentialAlgorithm::Argon2id => "ARGON2ID",
    }
}

/// Preparación Argon2id de credenciales nuevas.
///
/// Se ejecuta en `spawn_blocking` porque el coste de memoria del algoritmo no
/// debe ocupar un hilo del runtime asíncrono. La política se inyecta de forma
/// explícita: no existe un valor comercial por defecto.
pub struct LocalPasswordHasher {
    policy: pos_credentials::Argon2Policy,
}

impl LocalPasswordHasher {
    /// Política LAB_V1: reconocida y medida, **no ratificada** (D-AUTH-09).
    ///
    /// Es la misma que usa el asistente de primera instalación, de modo que un
    /// administrador creado aquí y otro creado por el asistente comparten
    /// política y son verificables por el mismo gate.
    pub fn lab_v1() -> Self {
        Self {
            policy: pos_credentials::Argon2Policy::LAB_V1,
        }
    }
}

impl PasswordHasher for LocalPasswordHasher {
    async fn prepare(
        &self,
        credential_id: EntityId,
        user_id: EntityId,
        password: &str,
        now: pos_core::identity::UtcSeconds,
    ) -> Result<LocalCredential, ApplicationError> {
        let policy = self.policy;
        let password = zeroize::Zeroizing::new(password.to_owned());
        tokio::task::spawn_blocking(move || {
            let secret_hash = pos_credentials::hash_password(password.as_str(), policy)
                .map_err(|_| ApplicationError::Storage)?;
            let parameters = pos_core::identity::CredentialParameters::new(&serde_json::json!({
                "m_cost": policy.memory_kib(),
                "t_cost": policy.iterations(),
                "p_cost": policy.lanes(),
            }))
            .map_err(|_| ApplicationError::Storage)?;
            LocalCredential::create(pos_core::identity::NewCredential {
                id: credential_id,
                user_id,
                kind: CredentialKind::Password,
                algorithm: CredentialAlgorithm::Argon2id,
                secret_hash,
                parameters,
                policy_id: policy.id().into(),
                created_at: now,
            })
            .map_err(|_| ApplicationError::Storage)
        })
        .await
        .map_err(|_| ApplicationError::Storage)?
    }
}
