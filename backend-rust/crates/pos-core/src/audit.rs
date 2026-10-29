//! Dominio puro de auditoría Local.
//!
//! Estos tipos representan un hecho administrativo append-only antes de conocer
//! el esquema SQLite definitivo. No resuelven autenticación, RBAC ni sesiones:
//! la infraestructura debe construir la atribución solo después de verificar un
//! principal y sus scopes actuales.

use std::fmt;

use serde_json::Value;

use crate::organization_branch::{EntityId, java_trim};

/// Límite máximo del JSON canónico de metadata, en bytes UTF-8.
pub const MAX_METADATA_SERIALIZED_BYTES: usize = 16 * 1024;

/// Profundidad máxima técnica del árbol JSON de metadata, incluyendo la raíz.
pub const MAX_METADATA_DEPTH: usize = 32;

const FORBIDDEN_KEY_TOKENS: &[&str] = &[
    "password",
    "token",
    "authorization",
    "jwt",
    "secret",
    "privatekey",
    "credential",
    "apikey",
];

/// Errores de validación de la foundation de auditoría, sin incluir valores sensibles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditError {
    /// La metadata debe comenzar en un objeto JSON.
    MetadataRootMustBeObject,
    /// Una clave de metadata se normaliza a un término sensible prohibido.
    SensitiveMetadataKey,
    /// La metadata excede la profundidad acotada del contrato.
    MetadataTooDeep,
    /// La metadata contiene un número no finito o no representable en JSON.
    InvalidMetadataNumber,
    /// El JSON canónico excede 16 KiB; no se trunca.
    MetadataTooLarge,
    /// Una acción no cumple el identificador estable de auditoría.
    InvalidAction,
    /// Un tipo de entidad no cumple el identificador estable de auditoría.
    InvalidEntityType,
    /// La razón no es texto válido o excede el límite Java vigente.
    InvalidReason,
    /// La fecha no es un instante UTC RFC 3339 acotado.
    InvalidOccurredAt,
}

impl fmt::Display for AuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MetadataRootMustBeObject => "la metadata de auditoría debe ser un objeto JSON",
            Self::SensitiveMetadataKey => "la metadata contiene una clave sensible",
            Self::MetadataTooDeep => "la metadata excede la profundidad permitida",
            Self::InvalidMetadataNumber => "la metadata contiene un número JSON no representable",
            Self::MetadataTooLarge => "la metadata excede el límite serializado",
            Self::InvalidAction => "la acción de auditoría no es válida",
            Self::InvalidEntityType => "el tipo de entidad de auditoría no es válido",
            Self::InvalidReason => "la razón de auditoría no es válida",
            Self::InvalidOccurredAt => "la fecha de auditoría no es UTC RFC 3339 válida",
        })
    }
}

impl std::error::Error for AuditError {}

/// Identidad estable de un hecho de auditoría generado fuera de la base de datos.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuditEventId(EntityId);

impl AuditEventId {
    /// Construye la identidad desde un UUID canónico ya validado.
    pub fn new(value: EntityId) -> Self {
        Self(value)
    }

    /// Devuelve el UUID canónico persistible.
    pub fn as_entity_id(&self) -> &EntityId {
        &self.0
    }
}

/// Actor explícito del hecho auditado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditActor {
    /// Principal humano autenticado y atribuible a un usuario Local.
    User {
        /// Identidad del usuario atribuida por el futuro adaptador de Auth.
        user_id: EntityId,
    },
    /// Camino interno autorizado, sin usuario técnico artificial.
    System {
        /// Procedencia controlada que justificó el actor SYSTEM.
        provenance: SystemProvenance,
    },
}

impl AuditActor {
    /// Construye un actor USER previamente verificado por la frontera de Auth.
    pub fn user(user_id: EntityId) -> Self {
        Self::User { user_id }
    }

    /// Construye un actor SYSTEM exclusivamente para un camino interno explícito.
    pub fn system(provenance: SystemProvenance) -> Self {
        Self::System { provenance }
    }

    /// Indica si el actor requiere una FK futura contra el usuario Local.
    pub fn user_id(&self) -> Option<&EntityId> {
        match self {
            Self::User { user_id } => Some(user_id),
            Self::System { .. } => None,
        }
    }

    /// Etiqueta cerrada para la futura constraint física.
    pub fn actor_type(&self) -> &'static str {
        match self {
            Self::User { .. } => "USER",
            Self::System { .. } => "SYSTEM",
        }
    }
}

/// Procedencia autorizada de un evento SYSTEM.
///
/// No representa una sesión ni permite degradar un principal autenticado a
/// SYSTEM. El adaptador Auth debe fallar cerrado si no puede atribuir un USER.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemProvenance {
    /// Provisioning inicial protegido, antes de que exista un usuario autenticado.
    SecureBootstrap,
    /// Mantenimiento interno aprobado.
    ControlledMaintenance,
    /// Flujo interno de la aplicación autorizado de forma explícita.
    InternalWorkflow,
}

/// Acción estable de auditoría, por ejemplo `organization.updated`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditAction(String);

impl AuditAction {
    /// Conserva la validación Java: el valor no puede quedar vacío tras
    /// `String.trim()` ni contener NUL. El catálogo y el límite físico Local
    /// permanecen sujetos a la ficha normativa pendiente.
    pub fn new(value: &str) -> Result<Self, AuditError> {
        if java_trim(value).is_empty() || value.contains('\0') {
            return Err(AuditError::InvalidAction);
        }
        Ok(Self(value.to_owned()))
    }

    /// Devuelve el identificador persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Tipo estable de la entidad afectada, por ejemplo `branch_membership`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEntityType(String);

impl AuditEntityType {
    /// Conserva la validación Java: el valor no puede quedar vacío tras
    /// `String.trim()` ni contener NUL. Los nombres estables se controlan en
    /// los casos de uso y el catálogo físico pendiente, no por una gramática
    /// Rust más restrictiva que la fuente vigente.
    pub fn new(value: &str) -> Result<Self, AuditError> {
        if java_trim(value).is_empty() || value.contains('\0') {
            return Err(AuditError::InvalidEntityType);
        }
        Ok(Self(value.to_owned()))
    }

    /// Devuelve el identificador persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Razón opcional de una corrección, devolución o ajuste futuro.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditReason(String);

impl AuditReason {
    /// Conserva texto no NUL. Java no valida razón en el objeto de aplicación;
    /// el límite/semántica Local deberá aprobarse con la ficha física.
    pub fn new(value: &str) -> Result<Self, AuditError> {
        if value.contains('\0') {
            return Err(AuditError::InvalidReason);
        }
        Ok(Self(value.to_owned()))
    }

    /// Devuelve la razón segura para persistir.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Instante UTC RFC 3339 que será sellado por un reloj de infraestructura.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditOccurredAt(String);

impl AuditOccurredAt {
    /// Valida el perfil UTC RFC 3339 `YYYY-MM-DDTHH:MM:SS[.fracción]Z`.
    pub fn new(value: &str) -> Result<Self, AuditError> {
        if !is_utc_rfc3339(value) {
            return Err(AuditError::InvalidOccurredAt);
        }
        Ok(Self(value.to_owned()))
    }

    /// Devuelve la representación UTC persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Metadata JSON segura, validada y serializada de forma determinista.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditMetadata {
    value: Value,
    canonical_json: String,
}

impl AuditMetadata {
    /// Construye metadata desde un objeto JSON; arrays y objetos anidados son válidos.
    ///
    /// Todas las claves se inspeccionan recursivamente antes de conservar el
    /// árbol. La serialización resultante es canónica, no se trunca y queda
    /// limitada a [`MAX_METADATA_SERIALIZED_BYTES`].
    pub fn new(value: Value) -> Result<Self, AuditError> {
        if !value.is_object() {
            return Err(AuditError::MetadataRootMustBeObject);
        }
        validate_metadata_value(&value, 1)?;
        let canonical_json = canonical_json(&value);
        if canonical_json.len() > MAX_METADATA_SERIALIZED_BYTES {
            return Err(AuditError::MetadataTooLarge);
        }
        Ok(Self {
            value,
            canonical_json,
        })
    }

    /// Construye metadata explícitamente vacía; al persistir debe representarse como `NULL`.
    pub fn empty() -> Self {
        Self {
            value: Value::Object(Default::default()),
            canonical_json: "{}".to_owned(),
        }
    }

    /// Indica si la metadata no tiene campos y puede persistirse como `NULL`.
    pub fn is_empty(&self) -> bool {
        self.value
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
    }

    /// Devuelve el árbol validado. `Value` no comparte mutabilidad con el input original.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Devuelve el JSON canónico que se debe persistir sin transformaciones posteriores.
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }
}

/// Scope de negocio que la futura frontera Auth debe validar al atribuir un evento.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditScope {
    /// Organización obligatoria del evento.
    pub organization_id: EntityId,
    /// Sucursal opcional; si existe debe pertenecer a `organization_id`.
    pub branch_id: Option<EntityId>,
}

/// Fallo seguro al intentar atribuir un actor desde Auth/RBAC.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditAttributionError {
    /// No existe un principal autenticado apto para una operación de usuario.
    Unauthenticated,
    /// El principal autenticado no se puede atribuir a un usuario Local verificable.
    UnattributablePrincipal,
    /// El principal no está autorizado en el scope solicitado.
    ScopeDenied,
    /// Un camino externo intentó declararse SYSTEM sin autorización interna.
    SystemPathDenied,
}

impl fmt::Display for AuditAttributionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unauthenticated => "no hay principal autenticado para atribuir la auditoría",
            Self::UnattributablePrincipal => "el principal no permite atribución de auditoría",
            Self::ScopeDenied => "el principal no tiene scope para la auditoría",
            Self::SystemPathDenied => "el camino SYSTEM no está autorizado",
        })
    }
}

impl std::error::Error for AuditAttributionError {}

/// Puerto futuro de atribución central, enlazado a Auth/RBAC DB-authoritative.
///
/// El adaptador deberá recibir la identidad desde una sesión verificada, comprobar
/// membership, permiso y estado actual en el mismo límite de consistencia de la
/// operación, y fallar cerrado. `local_session_id` no forma parte aún de este
/// contrato: su FK, lifecycle y obligatoriedad continúan pendientes de aprobación.
pub trait AuditActorResolver {
    /// Resuelve un actor para el scope de la operación sin aceptar datos del cliente.
    fn resolve_actor(&self, scope: &AuditScope) -> Result<AuditActor, AuditAttributionError>;
}

/// Reloj inyectable para sellar un hecho después de que Auth haya atribuido el actor.
pub trait AuditClock {
    /// Devuelve un instante UTC válido o falla antes de abrir/confirmar la UoW.
    fn now(&self) -> Result<AuditOccurredAt, AuditError>;
}

/// Hecho de auditoría completo, inmutable y aún independiente del esquema Local.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditEventRecord {
    /// UUID estable del evento.
    pub id: AuditEventId,
    /// Owner obligatorio del evento.
    pub organization_id: EntityId,
    /// Sucursal opcional; el futuro DDL debe aplicar FK compuesta same-org.
    pub branch_id: Option<EntityId>,
    /// Actor previamente atribuido por la frontera de autenticación o un flujo interno.
    pub actor: AuditActor,
    /// Acción estable.
    pub action: AuditAction,
    /// Tipo estable de entidad.
    pub entity_type: AuditEntityType,
    /// Identidad opcional de la entidad principal afectada.
    pub entity_id: Option<EntityId>,
    /// Razón opcional.
    pub reason: Option<AuditReason>,
    /// Contexto seguro; vacía equivale a ausencia persistida.
    pub metadata: AuditMetadata,
    /// Instante UTC entregado por un reloj inyectado.
    pub occurred_at: AuditOccurredAt,
}

fn validate_metadata_value(value: &Value, depth: usize) -> Result<(), AuditError> {
    if depth > MAX_METADATA_DEPTH {
        return Err(AuditError::MetadataTooDeep);
    }
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) => {
            if number.as_i64().is_some() || number.as_u64().is_some() {
                return Ok(());
            }
            if number.as_f64().is_some_and(f64::is_finite) {
                Ok(())
            } else {
                Err(AuditError::InvalidMetadataNumber)
            }
        }
        Value::Array(values) => values
            .iter()
            .try_for_each(|entry| validate_metadata_value(entry, depth + 1)),
        Value::Object(values) => values.iter().try_for_each(|(key, entry)| {
            validate_metadata_key(key)?;
            validate_metadata_value(entry, depth + 1)
        }),
    }
}

fn validate_metadata_key(key: &str) -> Result<(), AuditError> {
    let normalized: String = key
        .chars()
        .flat_map(char::to_lowercase)
        .filter(char::is_ascii_alphanumeric)
        .collect();
    if FORBIDDEN_KEY_TOKENS
        .iter()
        .any(|token| normalized.contains(token))
    {
        return Err(AuditError::SensitiveMetadataKey);
    }
    Ok(())
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.to_string(),
        Value::Array(values) => {
            let entries = values.iter().map(canonical_json).collect::<Vec<_>>();
            format!("[{}]", entries.join(","))
        }
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
            let encoded = entries
                .into_iter()
                .map(|(key, entry)| {
                    format!("{}:{}", Value::String(key.clone()), canonical_json(entry))
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", encoded.join(","))
        }
    }
}

fn is_utc_rfc3339(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
        || bytes.last() != Some(&b'Z')
    {
        return false;
    }
    let fixed_digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    if fixed_digits
        .into_iter()
        .any(|index| !bytes[index].is_ascii_digit())
    {
        return false;
    }
    let year = number(bytes, 0, 4);
    let month = number(bytes, 5, 7);
    let day = number(bytes, 8, 10);
    let hour = number(bytes, 11, 13);
    let minute = number(bytes, 14, 16);
    let second = number(bytes, 17, 19);
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return false;
    }
    match bytes.get(19) {
        Some(b'Z') => true,
        Some(b'.') => bytes.len() > 21 && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit),
        _ => false,
    }
}

fn number(bytes: &[u8], start: usize, end: usize) -> u32 {
    bytes[start..end]
        .iter()
        .fold(0_u32, |value, digit| value * 10 + u32::from(digit - b'0'))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}
