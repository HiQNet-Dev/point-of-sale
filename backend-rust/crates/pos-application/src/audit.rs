//! Auditoría obligatoria dentro de la unidad de trabajo.
//!
//! La construcción del evento es pura; la escritura se delega en un puerto que
//! el adaptador implementa sobre **la misma conexión transaccional**. Así una
//! mutación auditada no puede confirmarse por separado de su evento.
//!
//! # Garantías
//!
//! - La atribución procede de [`VerifiedPrincipal`]; nunca de la entrada.
//! - La metadata se valida y canonicaliza antes de tocar la base de datos.
//! - Un fallo de escritura devuelve [`ApplicationError::Audit`] y obliga a
//!   revertir la transacción completa.

use std::future::Future;

use pos_core::{
    audit::{
        AuditAction, AuditEntityType, AuditEventId, AuditEventRecord, AuditMetadata,
        AuditOccurredAt,
    },
    identity::VerifiedPrincipal,
    organization_branch::{ApplicationError, EntityId, IdentityGenerator},
};

/// Puerto de escritura de auditoría compartido con la transacción de negocio.
pub trait AuditAppender {
    /// Inserta el evento en la transacción ya abierta.
    fn append_audit_event(
        &mut self,
        event: &AuditEventRecord,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}

/// Construye el evento auditado sin generar identidad propia.
///
/// # Errors
/// Devuelve [`ApplicationError::Audit`] si la acción, el tipo de entidad o la
/// metadata incumplen el contrato.
#[allow(clippy::too_many_arguments)]
pub fn build_event(
    event_id: EntityId,
    principal: &VerifiedPrincipal,
    action: &str,
    entity_type: &str,
    entity_id: EntityId,
    metadata: serde_json::Value,
    occurred_at: &AuditOccurredAt,
) -> Result<AuditEventRecord, ApplicationError> {
    Ok(AuditEventRecord::from_verified_principal(
        AuditEventId::new(event_id),
        principal,
        AuditAction::new(action).map_err(|_| ApplicationError::Audit)?,
        AuditEntityType::new(entity_type).map_err(|_| ApplicationError::Audit)?,
        Some(entity_id),
        AuditMetadata::new(metadata).map_err(|_| ApplicationError::Audit)?,
        occurred_at.clone(),
    ))
}

/// Genera la identidad del evento y lo escribe en la transacción actual.
///
/// # Errors
/// Devuelve [`ApplicationError::Audit`] si el evento o la escritura fallan.
#[allow(clippy::too_many_arguments)]
pub async fn append_audit<A, I>(
    sink: &mut A,
    identities: &mut I,
    principal: &VerifiedPrincipal,
    action: &str,
    entity_type: &str,
    entity_id: EntityId,
    metadata: serde_json::Value,
    occurred_at: &AuditOccurredAt,
) -> Result<(), ApplicationError>
where
    A: AuditAppender + Send,
    I: IdentityGenerator,
{
    let event = build_event(
        identities.next_id()?,
        principal,
        action,
        entity_type,
        entity_id,
        metadata,
        occurred_at,
    )?;
    sink.append_audit_event(&event).await
}

/// Escribe un evento con identidad ya generada por el caso de uso.
///
/// Se usa cuando la identidad del evento y la de la entidad deben compartir el
/// mismo generador en un orden determinista.
///
/// # Errors
/// Devuelve [`ApplicationError::Audit`] si el evento o la escritura fallan.
#[allow(clippy::too_many_arguments)]
pub async fn append_audit_with_id<A>(
    sink: &mut A,
    event_id: EntityId,
    principal: &VerifiedPrincipal,
    action: &str,
    entity_type: &str,
    entity_id: EntityId,
    metadata: serde_json::Value,
    occurred_at: &AuditOccurredAt,
) -> Result<(), ApplicationError>
where
    A: AuditAppender + Send,
{
    let event = build_event(
        event_id,
        principal,
        action,
        entity_type,
        entity_id,
        metadata,
        occurred_at,
    )?;
    sink.append_audit_event(&event).await
}
