//! Contratos de entrada y salida del caso de uso de autenticación.
//!
//! Ninguno transporta actor, organización final, permisos ni el digest de
//! sesión. El handle viaja solo en la respuesta de login y el shell nativo es su
//! único custodio.

use std::collections::BTreeSet;

use pos_core::{
    audit::AuditOccurredAt,
    identity::{NormalizedEmail, UtcSeconds},
    organization_branch::EntityId,
};

/// Solicitud de autenticación por contraseña.
#[derive(Clone, Debug)]
pub struct LoginRequest {
    /// Email normalizado por el value object; nunca se compara en crudo.
    pub email: NormalizedEmail,
    /// Instante UTC de la comprobación y del evento.
    pub now: UtcSeconds,
    /// Instante RFC 3339 para la auditoría.
    pub occurred_at: AuditOccurredAt,
    /// Vigencia solicitada de la nueva sesión, en segundos.
    pub session_ttl_seconds: i64,
}

/// Identidad pública de una sesión autenticada.
///
/// Solo contiene datos no secretos y los permisos ya resueltos en SQLite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionIdentity {
    /// Usuario autenticado.
    pub user_id: EntityId,
    /// Organización derivada del usuario.
    pub organization_id: EntityId,
    /// Nombre visible.
    pub display_name: String,
    /// Email normalizado.
    pub email: String,
    /// Sucursal operativa seleccionada, si existe.
    pub branch_id: Option<EntityId>,
    /// Permisos efectivos del scope actual.
    pub permissions: BTreeSet<String>,
}

/// Resultado de un login correcto.
#[derive(Clone, Debug)]
pub struct AuthenticatedSession {
    /// Identidad y permisos vigentes.
    pub identity: SessionIdentity,
    /// Handle opaco de sesión; solo la capa nativa debe conservarlo.
    pub handle: String,
}
