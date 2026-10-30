//! Escritor de auditoría sobre la tabla candidata `audit_event`.
//!
//! **No productivo.** La tabla `audit_event` vive todavía en
//! `backend-rust/proposals/local-auth/0006_audit_event.proposed.sql` y no está
//! cargada por `pos-local` ni por SQLx. Este módulo escribe contra ese esquema
//! candidato para que, una vez aprobado D-AUTH-07/08, la promoción consista en
//! mover la migración y sustituir el writer de fixture, sin reescribir lógica.
//!
//! # Garantías
//!
//! - Se ejecuta **solo** sobre la conexión de una [`SqliteTransaction`] abierta
//!   por [`SqliteUnitOfWork`]: la mutación y su evento comparten commit.
//! - Un fallo devuelve error y deja que el llamador revierta; nunca se confirma
//!   la mutación sin su evento.
//! - `USER` exige `actor_user_id` y admite `session_id`; `SYSTEM` no admite
//!   ninguno de los dos. La atribución la produce `pos-core`; este writer no
//!   puede fabricar actores.
//! - `metadata_json` se guarda como JSON canónico ya validado (≤16 KiB) o
//!   `NULL` si está vacía. No se trunca nunca.
//! - El scope se limita a la organización del evento; la FK compuesta de
//!   sucursal y sesión la aplica el esquema.
//!
//! El historial es append-only por trigger del esquema, no por criptografía: un
//! administrador del archivo puede desactivarlo. Esa limitación está registrada
//! en el expediente 1M.

use pos_core::{audit::AuditEventRecord, organization_branch::EntityId};
use sqlx::SqliteConnection;

use crate::{ApplicationError, SqliteTransaction};

/// Sentencia única de inserción canónica.
const INSERT_AUDIT_EVENT_SQL: &str = "INSERT INTO audit_event (\
     id, organization_id, branch_id, actor_type, actor_user_id, session_id, \
     action, entity_type, entity_id, reason, metadata_json, occurred_at, event_version\
 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

/// Escritor de auditoría sin estado; opera sobre la conexión de la transacción.
#[derive(Clone, Copy, Debug, Default)]
pub struct SqliteAuditWriter;

impl SqliteAuditWriter {
    /// Añade el evento a la transacción actual.
    ///
    /// No confirma ni revierte: el commit pertenece al caso de uso que abrió la
    /// [`SqliteUnitOfWork`].
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Audit`] si la atribución es incoherente o si
    /// SQLite rechaza la fila (FK, `CHECK`, trigger append-only).
    pub async fn append(
        transaction: &mut SqliteTransaction,
        event: &AuditEventRecord,
    ) -> Result<(), ApplicationError> {
        Self::append_on(transaction.connection(), event).await
    }

    /// Variante sobre una conexión concreta, útil cuando el llamador ya tiene la
    /// conexión transaccional prestada.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Audit`] en las mismas condiciones que
    /// [`Self::append`].
    pub async fn append_on(
        connection: &mut SqliteConnection,
        event: &AuditEventRecord,
    ) -> Result<(), ApplicationError> {
        validate_attribution(event)?;
        let metadata = if event.metadata_json.is_empty() {
            None
        } else {
            Some(event.metadata_json.canonical_json())
        };
        let event_version = i64::from(event.event_version.get());
        sqlx::query(INSERT_AUDIT_EVENT_SQL)
            .bind(event.id.as_entity_id().as_str())
            .bind(event.organization_id.as_str())
            .bind(event.branch_id.as_ref().map(EntityId::as_str))
            .bind(event.actor.actor_type())
            .bind(event.actor.user_id().map(EntityId::as_str))
            .bind(event.session_id.as_ref().map(EntityId::as_str))
            .bind(event.action.as_str())
            .bind(event.entity_type.as_str())
            .bind(event.entity_id.as_ref().map(EntityId::as_str))
            .bind(
                event
                    .reason
                    .as_ref()
                    .map(pos_core::audit::AuditReason::as_str),
            )
            .bind(metadata)
            .bind(event.occurred_at.as_str())
            .bind(event_version)
            .execute(connection)
            .await
            .map_err(|_| ApplicationError::Audit)?;
        Ok(())
    }
}

/// Rechaza atribuciones que el esquema también rechazaría, sin esperar al SQL.
fn validate_attribution(event: &AuditEventRecord) -> Result<(), ApplicationError> {
    if event.actor.actor_type() == "USER" {
        if event.actor.user_id().is_none() {
            return Err(ApplicationError::Audit);
        }
    } else if event.actor.user_id().is_some() || event.session_id.is_some() {
        return Err(ApplicationError::Audit);
    }
    Ok(())
}
