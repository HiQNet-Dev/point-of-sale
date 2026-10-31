//! Adaptador de provisioning sobre el esquema candidato `0002`–`0007`.
//!
//! La coordinación y atribución SYSTEM pertenecen a Application. Este adaptador
//! solo persiste en la UoW existente, nunca confirma ni carga propuestas.

use pos_application::bootstrap::BootstrapTransaction;
pub use pos_application::bootstrap::{BootstrapRequest, provision_initial_installation};
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
        assign_role(connection, request, occurred_at).await?;
        complete_bootstrap(connection, request, occurred_at).await
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
         WHERE installation_id = ? AND state = 'PENDING'",
    )
    .bind(occurred_at)
    .bind(request.organization_id.as_str())
    .bind(request.administrator_id.as_str())
    .bind(request.administrator_role_id.as_str())
    .bind(occurred_at)
    .bind(request.installation_id.as_str())
    .execute(connection)
    .await
    .map_err(|_| ApplicationError::Storage)?;
    if updated.rows_affected() != 1 {
        return Err(ApplicationError::Storage);
    }
    Ok(())
}
