//! Adaptador de provisioning sobre el esquema candidato `0002`–`0007`.
//!
//! La coordinación y atribución SYSTEM pertenecen a Application. Este adaptador
//! solo persiste en la UoW existente, nunca confirma ni carga propuestas.

use pos_application::bootstrap::BootstrapTransaction;
pub use pos_application::bootstrap::{BootstrapRequest, provision_initial_installation};
use pos_core::identity::ProvisioningPurpose;
use pos_core::organization_branch::{ApplicationError, Organization};
use sqlx::SqliteConnection;

use crate::SqliteTransaction;

impl BootstrapTransaction for SqliteTransaction {
    async fn has_initial_identity(&mut self) -> Result<bool, ApplicationError> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM organization) OR EXISTS(SELECT 1 FROM installation_identity) \
             OR EXISTS(SELECT 1 FROM bootstrap_record) OR EXISTS(SELECT 1 FROM pos_user)",
        )
        .fetch_one(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)
    }

    async fn grant_administrator_catalog(
        &mut self,
        request: &BootstrapRequest,
    ) -> Result<(), ApplicationError> {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission_code) SELECT ?, code FROM permission",
        )
        .bind(request.administrator_role_id.as_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(())
    }

    async fn insert_initial_credential(
        &mut self,
        request: &BootstrapRequest,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        let credential = request.initial_credential.credential();
        sqlx::query(
            "INSERT INTO local_credential (id,user_id,organization_id,credential_kind,algorithm,secret_hash,params_json,policy_id,state,credential_version,failed_login_attempts,created_at,updated_at) \
             VALUES (?,?,?,'PASSWORD','ARGON2ID',?,?,?,'ACTIVE',1,0,?,?)",
        )
        .bind(credential.id().as_str())
        .bind(credential.user_id().as_str())
        .bind(request.organization_id.as_str())
        .bind(credential.secret_hash().expose_to_verifier())
        .bind(credential.parameters().canonical_json())
        .bind(credential.policy_id())
        .bind(occurred_at).bind(occurred_at)
        .execute(self.connection()).await.map_err(|_| ApplicationError::Storage)?;
        Ok(())
    }

    async fn complete_initial_bootstrap(
        &mut self,
        request: &BootstrapRequest,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        complete_bootstrap(self.connection(), request, occurred_at).await
    }

    async fn insert_initial_identity(
        &mut self,
        organization: &Organization,
        request: &BootstrapRequest,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        self.insert_organization(organization, occurred_at)
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let connection = self.connection();
        insert_installation(connection, request, occurred_at).await?;
        insert_pending_bootstrap(connection, request, occurred_at).await?;
        insert_administrator(connection, request, occurred_at).await?;
        insert_role(connection, request, occurred_at).await?;
        assign_role(connection, request, occurred_at).await
    }

    async fn consume_provisioning_authorization(
        &mut self,
        request: &BootstrapRequest,
        authorization_id: &pos_core::organization_branch::EntityId,
        presented_digest: &str,
        now_epoch: i64,
    ) -> Result<(), ApplicationError> {
        crate::provisioning::consume_derived(
            self,
            authorization_id,
            presented_digest,
            &request.installation_id,
            ProvisioningPurpose::Bootstrap,
            now_epoch,
        )
        .await
        .map(|_| ())
        .map_err(|_| ApplicationError::Storage)
    }

    async fn claim_provisional_installation(
        &mut self,
        request: &BootstrapRequest,
        now_epoch: i64,
    ) -> Result<(), ApplicationError> {
        let updated = sqlx::query(
            "UPDATE provisional_installation SET claimed_at = ?, claimed_installation_id = ? \
             WHERE id = ? AND claimed_at IS NULL",
        )
        .bind(now_epoch)
        .bind(request.installation_id.as_str())
        .bind(request.installation_id.as_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        if updated.rows_affected() != 1 {
            return Err(ApplicationError::Storage);
        }
        Ok(())
    }
}

async fn insert_installation(
    connection: &mut SqliteConnection,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<(), ApplicationError> {
    sqlx::query(
        "INSERT INTO installation_identity (id, organization_id, public_key_fingerprint, initialized_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(request.installation_id.as_str())
    .bind(request.organization_id.as_str())
    .bind(&request.installation_fingerprint)
    .bind(occurred_at)
    .bind(occurred_at)
    .execute(connection)
    .await
    .map_err(|_| ApplicationError::Storage)?;
    Ok(())
}

async fn insert_pending_bootstrap(
    connection: &mut SqliteConnection,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<(), ApplicationError> {
    sqlx::query(
        "INSERT INTO bootstrap_record (installation_id, state, updated_at) VALUES (?, 'PENDING', ?)",
    )
    .bind(request.installation_id.as_str())
    .bind(occurred_at)
    .execute(connection)
    .await
    .map_err(|_| ApplicationError::Storage)?;
    Ok(())
}

async fn insert_administrator(
    connection: &mut SqliteConnection,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<(), ApplicationError> {
    sqlx::query(
        "INSERT INTO pos_user (id, organization_id, display_name, email, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'ACTIVE', 0, ?, ?)",
    )
    .bind(request.administrator_id.as_str())
    .bind(request.organization_id.as_str())
    .bind(request.administrator_name.as_str())
    .bind(request.administrator_email.as_str())
    .bind(occurred_at)
    .bind(occurred_at)
    .execute(connection)
    .await
    .map_err(|_| ApplicationError::Storage)?;
    Ok(())
}

async fn insert_role(
    connection: &mut SqliteConnection,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<(), ApplicationError> {
    sqlx::query(
        "INSERT INTO role (id, organization_id, code, name, status, version, created_at, updated_at) \
         VALUES (?, ?, ?, 'Administrador', 'ACTIVE', 0, ?, ?)",
    )
    .bind(request.administrator_role_id.as_str())
    .bind(request.organization_id.as_str())
    .bind(request.administrator_role_code.as_str())
    .bind(occurred_at)
    .bind(occurred_at)
    .execute(connection)
    .await
    .map_err(|_| ApplicationError::Storage)?;
    Ok(())
}

async fn assign_role(
    connection: &mut SqliteConnection,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<(), ApplicationError> {
    sqlx::query(
        "INSERT INTO user_organization_role (organization_id, user_id, role_id, created_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(request.organization_id.as_str())
    .bind(request.administrator_id.as_str())
    .bind(request.administrator_role_id.as_str())
    .bind(occurred_at)
    .execute(connection)
    .await
    .map_err(|_| ApplicationError::Storage)?;
    Ok(())
}

async fn complete_bootstrap(
    connection: &mut SqliteConnection,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<(), ApplicationError> {
    let updated = sqlx::query(
        "UPDATE bootstrap_record SET state = 'COMPLETED', completed_at = ?, organization_id = ?, \
         administrator_user_id = ?, administrator_role_id = ?, bootstrap_version = 1, updated_at = ? \
         WHERE installation_id = ? AND state = 'PENDING' \
         AND EXISTS (SELECT 1 FROM local_credential WHERE user_id = ? AND organization_id = ? \
             AND credential_kind = 'PASSWORD' AND state = 'ACTIVE') \
         AND EXISTS (SELECT 1 FROM audit_event WHERE id = ? AND action = 'bootstrap.completed' \
             AND actor_type = 'SYSTEM' AND organization_id = ? AND entity_id = ?)",
    )
    .bind(occurred_at)
    .bind(request.organization_id.as_str())
    .bind(request.administrator_id.as_str())
    .bind(request.administrator_role_id.as_str())
    .bind(occurred_at)
    .bind(request.installation_id.as_str())
    .bind(request.administrator_id.as_str())
    .bind(request.organization_id.as_str())
    .bind(request.audit_event_id.as_entity_id().as_str())
    .bind(request.organization_id.as_str())
    .bind(request.installation_id.as_str())
    .execute(connection)
    .await
    .map_err(|_| ApplicationError::Storage)?;
    if updated.rows_affected() != 1 {
        return Err(ApplicationError::Storage);
    }
    Ok(())
}
