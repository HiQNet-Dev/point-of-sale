//! Escritor de auditoría sobre la tabla candidata `audit_event`.
//!
//! **No productivo.** La tabla `audit_event` vive todavía en
//! `backend-rust/proposals/local-auth/0006_audit_event.proposed.sql` y no está
//! cargada por `pos-local` ni por SQLx. Este módulo escribe contra ese esquema
//! candidato para que, una vez aprobado D-AUTH-07/08, la promoción consista en
//! mover la migración y activar el writer, sin reescribir lógica.
//!
//! # Frontera transaccional
//!
//! La única API pública recibe `&mut SqliteTransaction`. No existe variante que
//! acepte una conexión suelta o el pool: la mutación de negocio y su evento
//! comparten la misma conexión y el mismo commit, y un fallo de auditoría obliga
//! a revertir. No se abre ninguna conexión adicional durante la transacción.
//!
//! # Garantías
//!
//! - `USER` exige usuario y admite sesión; `SYSTEM` no admite ninguno de los dos.
//!   La atribución se construye en `pos-core` y ya no es mutable.
//! - `metadata_json` se guarda como JSON canónico ya validado (≤16 KiB) o `NULL`
//!   si está vacía; nunca se trunca.
//! - El historial es append-only por trigger del esquema, no por criptografía:
//!   un administrador del archivo puede desactivarlo (registrado en 1M).

use pos_core::{audit::AuditEventRecord, organization_branch::EntityId};
use sqlx::SqliteConnection;

use crate::{ApplicationError, SqliteTransaction};

/// Sentencia única de inserción canónica.
const INSERT_AUDIT_EVENT_SQL: &str = "INSERT INTO audit_event (\
     id, organization_id, branch_id, actor_type, actor_user_id, session_id, \
     action, entity_type, entity_id, reason, metadata_json, occurred_at, event_version\
 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

/// Escritor de auditoría sin estado; solo opera dentro de una unidad de trabajo.
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
        write(transaction.connection(), event).await
    }
}

/// Escritura interna sobre la conexión ya prestada por la transacción.
///
/// Privada del módulo: el resto del crate no puede invocarla sobre una conexión
/// en autocommit. Otros adaptadores que participen en la misma UoW reutilizan la
/// conexión a través de sus propios métodos transaccionales.
async fn write(
    connection: &mut SqliteConnection,
    event: &AuditEventRecord,
) -> Result<(), ApplicationError> {
    event.validate().map_err(|_| ApplicationError::Audit)?;
    let metadata = if event.metadata_json().is_empty() {
        None
    } else {
        Some(event.metadata_json().canonical_json())
    };
    let event_version = i64::from(event.event_version().get());
    sqlx::query(INSERT_AUDIT_EVENT_SQL)
        .bind(event.id().as_entity_id().as_str())
        .bind(event.organization_id().as_str())
        .bind(event.branch_id().map(EntityId::as_str))
        .bind(event.actor().actor_type())
        .bind(event.actor().user_id().map(EntityId::as_str))
        .bind(event.session_id().map(EntityId::as_str))
        .bind(event.action().as_str())
        .bind(event.entity_type().as_str())
        .bind(event.entity_id().map(EntityId::as_str))
        .bind(event.reason().map(pos_core::audit::AuditReason::as_str))
        .bind(metadata)
        .bind(event.occurred_at().as_str())
        .bind(event_version)
        .execute(connection)
        .await
        .map_err(|_| ApplicationError::Audit)?;
    Ok(())
}
