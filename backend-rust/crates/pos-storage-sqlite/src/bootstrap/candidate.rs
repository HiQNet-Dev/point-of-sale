//! Provisioning inicial sobre el esquema candidato `0002`–`0007`.
//!
//! **No productivo.** Las tablas viven en `proposals/local-auth/` y `pos-local`
//! no las carga. La función existe para que, tras la aprobación normativa, la
//! activación sea trasladar la migración y conectar este caso de uso.
//!
//! # Frontera de seguridad
//!
//! El evento `bootstrap.completed` exige [`BootstrapProvisioning`], que solo se
//! emite mediante [`pos_core::audit::authorize_bootstrap`] tras validar el
//! secreto de despliegue. Sin capacidad no se puede auditar el provisioning, y
//! sin auditoría la transacción se revierte: no queda una instalación parcial.
//!
//! # Orden
//!
//! `organization → installation_identity → bootstrap PENDING → pos_user → role →
//! user_organization_role → bootstrap COMPLETED → audit SYSTEM → COMMIT`.

use pos_core::{
    audit::{AuditEventId, AuditEventRecord, AuditOccurredAt, BootstrapProvisioning},
    identity::NormalizedEmail,
    organization_branch::{
        ApplicationError, BusinessCode, DisplayName, EntityId, NewOrganization, Organization,
    },
};
use sqlx::SqliteConnection;

use crate::{SqliteTransaction, audit::SqliteAuditWriter};

/// Datos del provisioning inicial.
#[derive(Clone, Debug)]
pub struct BootstrapRequest {
    /// Instalación que se está provisionando.
    pub installation_id: EntityId,
    /// Evento de auditoría del bootstrap.
    pub audit_event_id: AuditEventId,
    /// Identificador de la organización inicial.
    pub organization_id: EntityId,
    /// Organización inicial.
    pub organization: NewOrganization,
    /// Identificador del administrador inicial.
    pub administrator_id: EntityId,
    /// Nombre visible del administrador.
    pub administrator_name: DisplayName,
    /// Email normalizado del administrador.
    pub administrator_email: NormalizedEmail,
    /// Rol administrador de la organización.
    pub administrator_role_id: EntityId,
    /// Código del rol administrador (slug).
    pub administrator_role_code: BusinessCode,
    /// Huella pública de la instalación (la clave privada nunca se persiste).
    pub installation_fingerprint: String,
}

/// Provisiona la instalación completa dentro de la transacción abierta.
///
/// # Errors
/// Devuelve [`ApplicationError::Storage`] si SQLite rechaza cualquier paso y
/// [`ApplicationError::Audit`] si la auditoría no puede escribirse. El llamador
/// debe revertir en ambos casos.
pub async fn provision_initial_installation(
    transaction: &mut SqliteTransaction,
    capability: &BootstrapProvisioning,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<Organization, ApplicationError> {
    let organization = Organization::create(
        request.organization_id.clone(),
        request.organization.clone(),
    );
    // La organización se crea primero: la instalación la referencia.
    transaction
        .insert_organization(&organization, occurred_at)
        .await
        .map_err(|_| ApplicationError::Storage)?;
    let connection = transaction.connection();
    insert_installation(connection, request, occurred_at).await?;
    insert_pending_bootstrap(connection, request, occurred_at).await?;
    insert_administrator(connection, request, occurred_at).await?;
    insert_role(connection, request, occurred_at).await?;
    assign_role(connection, request, occurred_at).await?;
    complete_bootstrap(connection, request, occurred_at).await?;

    let event = AuditEventRecord::bootstrap_completed(
        capability,
        request.audit_event_id.clone(),
        organization.id().clone(),
        request.installation_id.clone(),
        AuditOccurredAt::new(occurred_at).map_err(|_| ApplicationError::Audit)?,
    )
    .map_err(|_| ApplicationError::Audit)?;
    SqliteAuditWriter::append(transaction, &event).await?;
    Ok(organization)
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
        // Un segundo intento no puede reabrir ni completar de nuevo.
        return Err(ApplicationError::Storage);
    }
    Ok(())
}
