use super::*;

/// Evento de auditoría aún independiente de su representación persistida.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEvent {
    /// Organización a la que pertenece el evento.
    pub organization_id: EntityId,
    /// Sucursal opcional afectada.
    pub branch_id: Option<EntityId>,
    /// Acción estable compatible con Java.
    pub action: &'static str,
    /// Tipo de entidad afectada.
    pub entity_type: &'static str,
    /// Entidad afectada.
    pub entity_id: EntityId,
    /// Campos modificados, sin valores potencialmente sensibles.
    pub changed_fields: Vec<&'static str>,
}

/// Marca temporal UTC entregada por un reloj de infraestructura.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtcTimestamp(String);

impl UtcTimestamp {
    /// Construye una marca no vacía y libre de NUL. La serialización RFC 3339 se
    /// comprueba en el adaptador que conoce su reloj concreto.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        if value.is_empty() || value.contains('\0') {
            return Err(DomainError::InvalidPersistedValue);
        }
        Ok(Self(value.to_owned()))
    }

    /// Valor persistible de la marca temporal.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
