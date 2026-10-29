//! Dominio de identidad Local y frontera de principal confiable.
//!
//! No contiene verificadores criptográficos, secretos, sesiones HTTP ni SQLx.
//! Un adaptador de Auth verifica una credencial opaca antes de consultar este
//! dominio; la autorización se resuelve desde datos vigentes, no desde el cliente.
//!
//! # Modelo canónico
//!
//! F1–10 §16.5 separa tres conceptos que no deben confundirse:
//!
//! 1. [`CredentialKind`]: finalidad (`PASSWORD` o `PIN`).
//! 2. [`CredentialAlgorithm`]: algoritmo del material (`LEGACY_BCRYPT` o
//!    `ARGON2ID`).
//! 3. [`CredentialState`]: lifecycle declarado de la credencial.
//!
//! La versión del material es [`LocalCredential::credential_version`] y la de la
//! sesión es [`LocalSession::session_version`]; ambas se usan también como
//! control de concurrencia optimista porque el contrato físico no define una
//! columna de revisión separada para esas tablas. Esa decisión sigue pendiente
//! de aprobación documental.

use std::{collections::BTreeSet, fmt, future::Future};

use serde_json::{Map, Value};

use crate::{
    audit::{AuditActor, AuditAttributionError},
    organization_branch::{DisplayName, EntityId, java_trim},
};

/// Máximo del JSON canónico de parámetros criptográficos.
pub const MAX_CREDENTIAL_PARAMETERS_BYTES: usize = 2048;
/// Profundidad máxima admitida en los parámetros criptográficos.
pub const MAX_CREDENTIAL_PARAMETERS_DEPTH: usize = 3;
/// Longitud máxima del hash codificado, alineada con Java V3 `password_hash`.
pub const MAX_SECRET_HASH_BYTES: usize = 255;

const FORBIDDEN_PARAMETER_TOKENS: [&str; 8] = [
    "password",
    "secret",
    "hash",
    "token",
    "authorization",
    "jwt",
    "salt",
    "credential",
];

/// Error de identidad sin hashes, handles ni otros materiales sensibles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// Email ausente, mal formado o fuera del límite heredado.
    InvalidEmail,
    /// Un nombre visible no satisface el value object común.
    InvalidDisplayName,
    /// Hash codificado inválido; nunca se devuelve su contenido.
    InvalidSecretHash,
    /// Identificador de política criptográfica inválido.
    InvalidCredentialPolicy,
    /// Parámetros criptográficos fuera del contrato estructurado.
    InvalidCredentialParameters,
    /// Operación incompatible con el estado actual de la credencial.
    InvalidCredentialState,
    /// Política de bloqueo fuera de rango.
    InvalidLockoutPolicy,
    /// Handle hash de sesión inválido; nunca se devuelve su contenido.
    InvalidSessionHash,
    /// Expiración de sesión no es posterior a su creación.
    InvalidSessionLifetime,
    /// Versión solicitada no coincide con el agregado actual.
    ConcurrentModification,
    /// La versión no puede incrementarse sin overflow.
    RevisionOverflow,
    /// La sesión no está activa en el instante solicitado.
    SessionNotActive,
    /// Usuario, owner o sesión no coincide con el scope solicitado.
    OwnershipMismatch,
    /// El usuario no está activo.
    UserInactive,
    /// El almacenamiento no pudo responder; nunca se interpreta como éxito.
    StorageUnavailable,
    /// El scope solicitado no existe, está inactivo o no pertenece al principal.
    ScopeDenied,
    /// Un permiso no es un código técnico estable.
    InvalidPermission,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEmail => "email inválido",
            Self::InvalidDisplayName => "nombre visible inválido",
            Self::InvalidSecretHash => "hash de credencial inválido",
            Self::InvalidCredentialPolicy => "política de credencial inválida",
            Self::InvalidCredentialParameters => "parámetros de credencial inválidos",
            Self::InvalidCredentialState => "estado de credencial incompatible",
            Self::InvalidLockoutPolicy => "política de bloqueo inválida",
            Self::InvalidSessionHash => "hash de sesión inválido",
            Self::InvalidSessionLifetime => "vigencia de sesión inválida",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "versión agotada",
            Self::SessionNotActive => "sesión no activa",
            Self::OwnershipMismatch => "identidad fuera de su organización",
            Self::UserInactive => "usuario inactivo",
            Self::StorageUnavailable => "almacenamiento no disponible",
            Self::ScopeDenied => "scope denegado",
            Self::InvalidPermission => "permiso inválido",
        })
    }
}
impl std::error::Error for IdentityError {}

/// Email normalizado como Java: `String.trim()` y minúsculas con semántica Root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedEmail(String);

impl NormalizedEmail {
    /// Valida la forma mínima heredada (`local@domain.tld`) y 254 unidades UTF-16.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        let normalized = java_trim(value).to_lowercase();
        let mut parts = normalized.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        if parts.next().is_some()
            || local.is_empty()
            || domain.is_empty()
            || !domain.contains('.')
            || normalized.chars().any(char::is_whitespace)
            || normalized.contains('\0')
            || normalized.encode_utf16().count() > 254
        {
            return Err(IdentityError::InvalidEmail);
        }
        Ok(Self(normalized))
    }

    /// Representación segura para búsquedas y unicidad por Organization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Estado administrativo del usuario Local.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PosUserStatus {
    /// Usuario disponible para autenticación y autorización.
    Active,
    /// Usuario conservado históricamente, sin capacidad operativa.
    Inactive,
}

/// Patch permitido: ni UUID ni Organization pueden modificarse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosUserPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre visible opcional.
    pub display_name: Option<DisplayName>,
    /// Email opcional ya validado.
    pub email: Option<NormalizedEmail>,
    /// Estado administrativo opcional.
    pub status: Option<PosUserStatus>,
}

/// Usuario Local, propietario de una sola Organization e independiente de credenciales.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosUser {
    id: EntityId,
    organization_id: EntityId,
    display_name: DisplayName,
    email: NormalizedEmail,
    status: PosUserStatus,
    revision: u64,
}

impl PosUser {
    /// Crea una identidad activa; credenciales se asocian posteriormente por UUID.
    pub fn create(
        id: EntityId,
        organization_id: EntityId,
        display_name: DisplayName,
        email: NormalizedEmail,
    ) -> Self {
        Self {
            id,
            organization_id,
            display_name,
            email,
            status: PosUserStatus::Active,
            revision: 0,
        }
    }

    /// Rehidrata un usuario persistido conservando estado y revisión.
    ///
    /// Uso exclusivo de adaptadores de persistencia; el email y el nombre ya
    /// llegan validados por sus value objects.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        display_name: DisplayName,
        email: NormalizedEmail,
        status: PosUserStatus,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            display_name,
            email,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Owner inmutable.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Nombre visible no secreto.
    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }
    /// Email normalizado.
    pub fn email(&self) -> &NormalizedEmail {
        &self.email
    }
    /// Estado actual.
    pub fn status(&self) -> PosUserStatus {
        self.status
    }
    /// Revisión optimista de la fila de usuario Java (`users.version`).
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Indica si la identidad puede autenticarse/autorizarse.
    pub fn is_active(&self) -> bool {
        self.status == PosUserStatus::Active
    }

    /// Aplica un patch atómico sin permitir mover al usuario ni modificar su UUID.
    pub fn patch(&mut self, patch: PosUserPatch) -> Result<bool, IdentityError> {
        if patch.expected_version != self.revision {
            return Err(IdentityError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.display_name {
            changed |= next.display_name != value;
            next.display_name = value;
        }
        if let Some(value) = patch.email {
            changed |= next.email != value;
            next.email = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(IdentityError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Finalidad de la credencial: columna canónica `credential_kind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    /// Contraseña de la cuenta.
    Password,
    /// PIN operativo de terminal.
    Pin,
}

/// Algoritmo del material criptográfico: columna canónica `algorithm`.
///
/// El dominio solo declara cuál es; nunca verifica ni produce el hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialAlgorithm {
    /// BCrypt heredado de Java, verificable y migrable solo tras aprobación.
    LegacyBcrypt,
    /// Argon2id candidato para material nuevo.
    Argon2id,
}

/// Lifecycle declarado de la credencial.
///
/// `ResetRequired` y `Disabled` son estados de dominio: la representación
/// física exacta del catálogo `local_credential.state` continúa pendiente de
/// aprobación, y en Java la desactivación administrativa vive en
/// `users.status`, no en una credencial independiente.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialState {
    /// Credencial apta para verificación por un adaptador autorizado.
    Active,
    /// Material no confiable: exige reset sin fallback.
    ResetRequired,
    /// Credencial deshabilitada por un flujo administrativo autorizado.
    Disabled,
}

/// Hash codificado del secreto; su `Debug` nunca revela contenido.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretHash(String);
impl SecretHash {
    /// Acepta un hash/PHC ya producido por un verificador autorizado.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || value.len() > MAX_SECRET_HASH_BYTES
            || value.contains(['\0', '\n', '\r'])
        {
            return Err(IdentityError::InvalidSecretHash);
        }
        Ok(Self(value.to_owned()))
    }
    /// Entrega el material solo al adaptador criptográfico dentro de una frontera de confianza.
    ///
    /// El hash codificado nunca debe registrarse, serializarse ni devolverse por
    /// transporte; `Debug` sigue redactado.
    pub fn expose_to_verifier(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SecretHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretHash(REDACTED)")
    }
}

/// Parámetros de verificación no secretos, serializados de forma determinista.
///
/// Se rechazan claves sensibles (contraseña, secreto, hash, token, salt…) para
/// que `params_json` no se convierta en un almacén de material criptográfico.
#[derive(Clone, Eq, PartialEq)]
pub struct CredentialParameters(String);
impl CredentialParameters {
    /// Valida un objeto JSON pequeño y no sensible.
    pub fn new(value: &Value) -> Result<Self, IdentityError> {
        if !value.is_object() {
            return Err(IdentityError::InvalidCredentialParameters);
        }
        if value.as_object().is_some_and(Map::is_empty) {
            return Err(IdentityError::InvalidCredentialParameters);
        }
        validate_parameters(value, 0)?;
        let canonical =
            serde_json::to_string(value).map_err(|_| IdentityError::InvalidCredentialParameters)?;
        if canonical.len() > MAX_CREDENTIAL_PARAMETERS_BYTES {
            return Err(IdentityError::InvalidCredentialParameters);
        }
        Ok(Self(canonical))
    }
    /// JSON canónico listo para `params_json`.
    pub fn canonical_json(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for CredentialParameters {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialParameters(")?;
        formatter.write_str(self.canonical_json())?;
        formatter.write_str(")")
    }
}

fn validate_parameters(value: &Value, depth: usize) -> Result<(), IdentityError> {
    if depth > MAX_CREDENTIAL_PARAMETERS_DEPTH {
        return Err(IdentityError::InvalidCredentialParameters);
    }
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                if is_sensitive_key(key) {
                    return Err(IdentityError::InvalidCredentialParameters);
                }
                validate_parameters(nested, depth + 1)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_parameters(item, depth + 1)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    FORBIDDEN_PARAMETER_TOKENS
        .iter()
        .any(|token| normalized.contains(token))
}

/// Política de bloqueo temporal inyectada desde configuración aprobada.
///
/// No se codifican valores por defecto: el dominio solo aplica la aritmética.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockoutPolicy {
    max_failed_attempts: u32,
    lockout_seconds: i64,
}
impl LockoutPolicy {
    /// Exige al menos un intento y una ventana positiva de bloqueo.
    pub fn new(max_failed_attempts: u32, lockout_seconds: i64) -> Result<Self, IdentityError> {
        if max_failed_attempts == 0 || lockout_seconds <= 0 {
            return Err(IdentityError::InvalidLockoutPolicy);
        }
        Ok(Self {
            max_failed_attempts,
            lockout_seconds,
        })
    }
    /// Intentos fallidos acumulados antes de bloquear.
    pub fn max_failed_attempts(&self) -> u32 {
        self.max_failed_attempts
    }
    /// Duración del bloqueo en segundos.
    pub fn lockout_seconds(&self) -> i64 {
        self.lockout_seconds
    }
}

/// Datos de alta de una credencial, separados de su lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCredential {
    /// UUID estable de la credencial.
    pub id: EntityId,
    /// Usuario owner, ya perteneciente a una Organization.
    pub user_id: EntityId,
    /// Finalidad declarada.
    pub kind: CredentialKind,
    /// Algoritmo declarado.
    pub algorithm: CredentialAlgorithm,
    /// Hash codificado producido externamente.
    pub secret_hash: SecretHash,
    /// Parámetros no secretos de verificación.
    pub parameters: CredentialParameters,
    /// Identificador de la política aprobada.
    pub policy_id: String,
    /// Instante UTC de creación.
    pub created_at: UtcSeconds,
}

/// Credencial Local separada de la identidad administrativa.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCredential {
    id: EntityId,
    user_id: EntityId,
    kind: CredentialKind,
    algorithm: CredentialAlgorithm,
    secret_hash: SecretHash,
    parameters: CredentialParameters,
    policy_id: String,
    state: CredentialState,
    credential_version: u64,
    failed_login_attempts: u32,
    locked_until: Option<UtcSeconds>,
    created_at: UtcSeconds,
    updated_at: UtcSeconds,
}
impl LocalCredential {
    /// Asocia una credencial activa a un usuario; la versión inicial es `1`.
    pub fn create(new: NewCredential) -> Result<Self, IdentityError> {
        validate_policy_id(&new.policy_id)?;
        Ok(Self {
            id: new.id,
            user_id: new.user_id,
            kind: new.kind,
            algorithm: new.algorithm,
            secret_hash: new.secret_hash,
            parameters: new.parameters,
            policy_id: new.policy_id,
            state: CredentialState::Active,
            credential_version: 1,
            failed_login_attempts: 0,
            locked_until: None,
            created_at: new.created_at,
            updated_at: new.created_at,
        })
    }

    /// Rehidrata una credencial persistida validando sus invariantes.
    ///
    /// Uso exclusivo de adaptadores de persistencia: no acepta datos de
    /// transporte y conserva la versión almacenada para el control de CAS.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        user_id: EntityId,
        kind: CredentialKind,
        algorithm: CredentialAlgorithm,
        secret_hash: SecretHash,
        parameters: CredentialParameters,
        policy_id: &str,
        state: CredentialState,
        credential_version: u64,
        failed_login_attempts: u32,
        locked_until: Option<UtcSeconds>,
        created_at: UtcSeconds,
        updated_at: UtcSeconds,
    ) -> Result<Self, IdentityError> {
        validate_policy_id(policy_id)?;
        if credential_version == 0 || updated_at < created_at {
            return Err(IdentityError::StorageUnavailable);
        }
        Ok(Self {
            id,
            user_id,
            kind,
            algorithm,
            secret_hash,
            parameters,
            policy_id: policy_id.to_owned(),
            state,
            credential_version,
            failed_login_attempts,
            locked_until,
            created_at,
            updated_at,
        })
    }
    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Usuario owner inmutable.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
    /// Finalidad de la credencial, no el algoritmo.
    pub fn kind(&self) -> CredentialKind {
        self.kind
    }
    /// Algoritmo declarado; la verificación real vive en el adaptador aprobado.
    pub fn algorithm(&self) -> CredentialAlgorithm {
        self.algorithm
    }
    /// Hash codificado, solo para persistencia y verificación de confianza.
    ///
    /// Nunca debe registrarse ni serializarse; `Debug` sigue redactado.
    pub fn secret_hash(&self) -> &SecretHash {
        &self.secret_hash
    }
    /// Parámetros no secretos de verificación.
    pub fn parameters(&self) -> &CredentialParameters {
        &self.parameters
    }
    /// Instante UTC de creación.
    pub fn created_at(&self) -> UtcSeconds {
        self.created_at
    }
    /// Instante UTC de última modificación.
    pub fn updated_at(&self) -> UtcSeconds {
        self.updated_at
    }
    /// Estado de ciclo de vida.
    pub fn state(&self) -> CredentialState {
        self.state
    }
    /// Política declarada, no secreta.
    pub fn policy_id(&self) -> &str {
        &self.policy_id
    }
    /// Versión del material criptográfico y control de concurrencia.
    pub fn credential_version(&self) -> u64 {
        self.credential_version
    }
    /// Intentos fallidos acumulados en la ventana actual.
    pub fn failed_login_attempts(&self) -> u32 {
        self.failed_login_attempts
    }
    /// Fin del bloqueo temporal, si existe.
    pub fn locked_until(&self) -> Option<UtcSeconds> {
        self.locked_until
    }
    /// Indica si la credencial está bloqueada en el instante recibido.
    pub fn is_locked(&self, now: UtcSeconds) -> bool {
        self.locked_until.is_some_and(|until| until > now)
    }
    /// Acepta verificación solo si está activa y sin bloqueo vigente.
    pub fn accepts_verification(&self, now: UtcSeconds) -> Result<(), IdentityError> {
        if self.state != CredentialState::Active {
            return Err(IdentityError::InvalidCredentialState);
        }
        if self.is_locked(now) {
            return Err(IdentityError::InvalidCredentialState);
        }
        Ok(())
    }
    /// Marca la credencial como reset requerido de manera idempotente.
    pub fn require_reset(&mut self, expected_version: u64) -> Result<bool, IdentityError> {
        self.ensure_version(expected_version)?;
        match self.state {
            CredentialState::ResetRequired => return Ok(false),
            CredentialState::Disabled => return Err(IdentityError::InvalidCredentialState),
            CredentialState::Active => {}
        }
        let version = self.next_version()?;
        self.state = CredentialState::ResetRequired;
        self.credential_version = version;
        Ok(true)
    }
    /// Deshabilita la credencial de forma idempotente.
    pub fn disable(&mut self, expected_version: u64) -> Result<bool, IdentityError> {
        self.ensure_version(expected_version)?;
        if self.state == CredentialState::Disabled {
            return Ok(false);
        }
        let version = self.next_version()?;
        self.state = CredentialState::Disabled;
        self.credential_version = version;
        Ok(true)
    }
    /// Registra un intento fallido replicando la semántica de `User.recordFailedLogin`.
    ///
    /// Una ventana de bloqueo ya vencida reinicia el contador. Devuelve `true`
    /// cuando el intento deja la credencial bloqueada.
    pub fn record_failed_login(
        &mut self,
        now: UtcSeconds,
        policy: LockoutPolicy,
        expected_version: u64,
    ) -> Result<bool, IdentityError> {
        self.ensure_version(expected_version)?;
        if self.state != CredentialState::Active {
            return Err(IdentityError::InvalidCredentialState);
        }
        let mut attempts = self.failed_login_attempts;
        let mut locked_until = self.locked_until;
        if locked_until.is_some_and(|until| until <= now) {
            attempts = 0;
            locked_until = None;
        }
        attempts = attempts
            .checked_add(1)
            .ok_or(IdentityError::RevisionOverflow)?;
        if attempts >= policy.max_failed_attempts() {
            let until = now
                .get()
                .checked_add(policy.lockout_seconds())
                .ok_or(IdentityError::RevisionOverflow)?;
            locked_until = Some(UtcSeconds::new(until)?);
        }
        let version = self.next_version()?;
        self.failed_login_attempts = attempts;
        self.locked_until = locked_until;
        self.credential_version = version;
        self.updated_at = now;
        Ok(locked_until.is_some_and(|until| until > now))
    }
    /// Limpia contador y bloqueo tras una verificación exitosa.
    pub fn record_successful_login(
        &mut self,
        now: UtcSeconds,
        expected_version: u64,
    ) -> Result<bool, IdentityError> {
        self.ensure_version(expected_version)?;
        if self.state != CredentialState::Active {
            return Err(IdentityError::InvalidCredentialState);
        }
        if self.failed_login_attempts == 0 && self.locked_until.is_none() {
            return Ok(false);
        }
        let version = self.next_version()?;
        self.failed_login_attempts = 0;
        self.locked_until = None;
        self.credential_version = version;
        self.updated_at = now;
        Ok(true)
    }
    /// Sustituye el material y la política; base de la transición BCrypt → Argon2id.
    pub fn rotate_material(
        &mut self,
        algorithm: CredentialAlgorithm,
        secret_hash: SecretHash,
        parameters: CredentialParameters,
        policy_id: &str,
        now: UtcSeconds,
        expected_version: u64,
    ) -> Result<bool, IdentityError> {
        validate_policy_id(policy_id)?;
        self.ensure_version(expected_version)?;
        if self.state == CredentialState::Disabled {
            return Err(IdentityError::InvalidCredentialState);
        }
        let version = self.next_version()?;
        self.algorithm = algorithm;
        self.secret_hash = secret_hash;
        self.parameters = parameters;
        self.policy_id = policy_id.to_owned();
        self.state = CredentialState::Active;
        self.failed_login_attempts = 0;
        self.locked_until = None;
        self.credential_version = version;
        self.updated_at = now;
        Ok(true)
    }
    fn ensure_version(&self, expected_version: u64) -> Result<(), IdentityError> {
        if expected_version != self.credential_version {
            return Err(IdentityError::ConcurrentModification);
        }
        Ok(())
    }
    fn next_version(&self) -> Result<u64, IdentityError> {
        self.credential_version
            .checked_add(1)
            .ok_or(IdentityError::RevisionOverflow)
    }
}

fn validate_policy_id(policy_id: &str) -> Result<(), IdentityError> {
    if policy_id.is_empty()
        || policy_id.len() > 64
        || !policy_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(IdentityError::InvalidCredentialPolicy);
    }
    Ok(())
}

/// Instant UTC lógico para decisiones de expiración, expresado como segundos Unix.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UtcSeconds(i64);
impl UtcSeconds {
    /// Construye un instante UTC no negativo.
    pub fn new(value: i64) -> Result<Self, IdentityError> {
        if value < 0 {
            return Err(IdentityError::InvalidSessionLifetime);
        }
        Ok(Self(value))
    }

    /// Segundos Unix.
    pub fn get(self) -> i64 {
        self.0
    }
}

/// Hash SHA-256 hexadecimal de un handle opaco; el handle crudo jamás entra al dominio.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionHash(String);
impl SessionHash {
    /// Valida exactamente 32 bytes SHA-256 codificados en hexadecimal minúsculo.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(IdentityError::InvalidSessionHash);
        }
        Ok(Self(value.to_owned()))
    }
    /// Uso exclusivo por repositorios/adaptadores de persistencia.
    ///
    /// Un hash SHA-256 no es un secreto, pero tampoco debe aparecer en
    /// respuestas, trazas ni mensajes de error.
    pub fn as_storage_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SessionHash {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("SessionHash(REDACTED)")
    }
}

/// Estado efectivo de una sesión Local en un instante.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalSessionState {
    /// Sesión vigente y no revocada.
    Active,
    /// Sesión revocada de forma explícita.
    Revoked,
    /// Sesión que agotó su vigencia temporal.
    Expired,
}

/// Sesión Local de handle opaco, ligada a usuario y Organization.
///
/// `organization_id` es ownership derivado de `pos_user`, no una columna
/// canónica de F1–10 §16.45: el adaptador debe obtenerlo del usuario y verificar
/// que coincide con el de la sesión antes de construir un principal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSession {
    id: EntityId,
    user_id: EntityId,
    organization_id: EntityId,
    session_hash: SessionHash,
    created_at: UtcSeconds,
    expires_at: UtcSeconds,
    revoked_at: Option<UtcSeconds>,
    session_version: u64,
}
impl LocalSession {
    /// Crea una sesión activa con `session_version` inicial `1`.
    pub fn create(
        id: EntityId,
        user_id: EntityId,
        organization_id: EntityId,
        session_hash: SessionHash,
        created_at: UtcSeconds,
        expires_at: UtcSeconds,
    ) -> Result<Self, IdentityError> {
        if expires_at <= created_at {
            return Err(IdentityError::InvalidSessionLifetime);
        }
        Ok(Self {
            id,
            user_id,
            organization_id,
            session_hash,
            created_at,
            expires_at,
            revoked_at: None,
            session_version: 1,
        })
    }
    /// Rehidrata una sesión persistida validando versión y vigencia.
    ///
    /// Uso exclusivo de adaptadores; no revive sesiones ni reinterpreta la
    /// revocación almacenada.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        user_id: EntityId,
        organization_id: EntityId,
        session_hash: SessionHash,
        created_at: UtcSeconds,
        expires_at: UtcSeconds,
        revoked_at: Option<UtcSeconds>,
        session_version: u64,
    ) -> Result<Self, IdentityError> {
        if expires_at <= created_at || session_version == 0 {
            return Err(IdentityError::StorageUnavailable);
        }
        Ok(Self {
            id,
            user_id,
            organization_id,
            session_hash,
            created_at,
            expires_at,
            revoked_at,
            session_version,
        })
    }

    /// Identidad estable que `audit_event.session_id` podrá referenciar.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Usuario owner inmutable.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
    /// Organization derivada del usuario owner.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Inicio UTC lógico.
    pub fn created_at(&self) -> UtcSeconds {
        self.created_at
    }
    /// Expiración UTC lógica.
    pub fn expires_at(&self) -> UtcSeconds {
        self.expires_at
    }
    /// Momento de revocación, si existe.
    pub fn revoked_at(&self) -> Option<UtcSeconds> {
        self.revoked_at
    }
    /// Versión de sesión, inicial `1`; también control de concurrencia.
    pub fn session_version(&self) -> u64 {
        self.session_version
    }
    /// Estado derivado: revocación gana a expiración.
    pub fn state_at(&self, now: UtcSeconds) -> LocalSessionState {
        if self.revoked_at.is_some() {
            LocalSessionState::Revoked
        } else if now >= self.expires_at {
            LocalSessionState::Expired
        } else {
            LocalSessionState::Active
        }
    }
    /// Verifica que el usuario recuperado sea realmente el owner de la sesión.
    pub fn ensure_owned_by(&self, user: &PosUser) -> Result<(), IdentityError> {
        if &self.user_id != user.id() || &self.organization_id != user.organization_id() {
            return Err(IdentityError::OwnershipMismatch);
        }
        Ok(())
    }
    /// Rechaza sesión expirada/revocada o de otro user/org.
    pub fn validate_for(
        &self,
        user_id: &EntityId,
        organization_id: &EntityId,
        now: UtcSeconds,
    ) -> Result<(), IdentityError> {
        if &self.user_id != user_id || &self.organization_id != organization_id {
            return Err(IdentityError::OwnershipMismatch);
        }
        if self.state_at(now) != LocalSessionState::Active {
            return Err(IdentityError::SessionNotActive);
        }
        Ok(())
    }
    /// Revoca con control de versión, sin revivir nunca una sesión.
    pub fn revoke(
        &mut self,
        expected_version: u64,
        now: UtcSeconds,
    ) -> Result<bool, IdentityError> {
        if expected_version != self.session_version {
            return Err(IdentityError::ConcurrentModification);
        }
        if self.revoked_at.is_some() {
            return Ok(false);
        }
        let version = self
            .session_version
            .checked_add(1)
            .ok_or(IdentityError::RevisionOverflow)?;
        self.revoked_at = Some(now);
        self.session_version = version;
        Ok(true)
    }
    /// Sólo para el repositorio de sesión; no se expone en DTOs/logs.
    #[allow(dead_code)]
    pub(crate) fn session_hash(&self) -> &SessionHash {
        &self.session_hash
    }
}

/// Código de permiso resuelto desde el catálogo vigente de base de datos.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PermissionCode(String);
impl PermissionCode {
    /// Acepta códigos técnicos ASCII separados por puntos, sin wildcard.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || value.len() > 64
            || value.split('.').any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
        {
            return Err(IdentityError::InvalidPermission);
        }
        Ok(Self(value.to_owned()))
    }
    /// Código para comparar una decisión de autorización.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Modo de evaluación de un scope de sucursal.
///
/// Java distingue `hasBranchPermission` de
/// `hasBranchPermissionIgnoringBranchStatus`: la variante de recuperación existe
/// solo para recursos ya abiertos en una sucursal desactivada (por ejemplo,
/// cerrar un turno de caja) y nunca habilita operaciones normales.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeMode {
    /// Operación normal: la pierna de sucursal exige sucursal ACTIVA.
    Normal,
    /// Recuperación autorizada: la pierna de sucursal no exige sucursal ACTIVA.
    Recovery,
}

/// Scope concreto de una operación: organización, sucursal opcional y modo.
///
/// El scope describe la petición; no concede nada por sí mismo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrincipalScope {
    organization_id: EntityId,
    branch_id: Option<EntityId>,
    mode: ScopeMode,
}
impl PrincipalScope {
    /// Construye un scope normal; la organización se resuelve aparte.
    pub fn new(organization_id: EntityId, branch_id: Option<EntityId>) -> Self {
        Self {
            organization_id,
            branch_id,
            mode: ScopeMode::Normal,
        }
    }
    /// Construye un scope de recuperación explícito para una sucursal concreta.
    pub fn recovery(organization_id: EntityId, branch_id: EntityId) -> Self {
        Self {
            organization_id,
            branch_id: Some(branch_id),
            mode: ScopeMode::Recovery,
        }
    }
    /// Organización del scope.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Sucursal del scope; `None` significa alcance de organización.
    pub fn branch_id(&self) -> Option<&EntityId> {
        self.branch_id.as_ref()
    }
    /// Modo de evaluación del scope.
    pub fn mode(&self) -> ScopeMode {
        self.mode
    }
}

/// Fallo de una decisión contextual de autorización.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationError {
    /// El scope solicitado no es el scope del principal autenticado.
    ScopeMismatch,
    /// El permiso no fue concedido para ese scope.
    PermissionDenied,
}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ScopeMismatch => "el scope no corresponde al principal",
            Self::PermissionDenied => "permiso denegado",
        })
    }
}
impl std::error::Error for AuthorizationError {}

/// Principal ya autenticado y scoped. Sus campos no son elegibles desde transporte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPrincipal {
    user_id: EntityId,
    session_id: EntityId,
    scope: PrincipalScope,
    permissions: BTreeSet<PermissionCode>,
}
impl VerifiedPrincipal {
    /// Usuario atribuido por sesión validada.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
    /// Sesión Local que correlacionará los hechos auditables.
    pub fn session_id(&self) -> &EntityId {
        &self.session_id
    }
    /// Scope exacto para el que se resolvieron los permisos.
    pub fn scope(&self) -> &PrincipalScope {
        &self.scope
    }
    /// Decide contextualmente: organización, sucursal y permiso en una sola llamada.
    ///
    /// Un permiso resuelto para la sucursal A nunca autoriza la sucursal B ni el
    /// alcance de organización, aunque compartan organización.
    pub fn authorize(
        &self,
        permission: &PermissionCode,
        scope: &PrincipalScope,
    ) -> Result<(), AuthorizationError> {
        if &self.scope != scope {
            return Err(AuthorizationError::ScopeMismatch);
        }
        if !self.permissions.contains(permission) {
            return Err(AuthorizationError::PermissionDenied);
        }
        Ok(())
    }
    /// Construye USER auditado sin recibir actor desde el cliente.
    pub fn audit_actor(&self) -> AuditActor {
        AuditActor::user(self.user_id.clone())
    }
}

/// Lecturas DB-authoritative requeridas para construir un principal.
///
/// Los métodos son asíncronos para permitir un adaptador SQLx real sin
/// `block_on` ni conexiones clonadas. El adaptador debe devolver únicamente
/// datos de identidad; nunca hashes, handles crudos ni material criptográfico.
pub trait LocalIdentityRepository {
    /// Busca por hash de sesión ya calculado por el adaptador criptográfico.
    fn find_session_by_hash(
        &mut self,
        hash: &SessionHash,
    ) -> impl Future<Output = Result<Option<LocalSession>, IdentityError>> + Send;
    /// Busca el usuario dentro de la Organization derivada de la sesión.
    fn find_user(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<Option<PosUser>, IdentityError>> + Send;
}

/// Resolución de permisos vigente en DB; no admite grants desde DTO/JWT.
///
/// Contrato alineado con Java:
///
/// - `branch_id = None`: solo grants de alcance organización (rol ACTIVO,
///   asignado al usuario, organización ACTIVA).
/// - `branch_id = Some(b)`: **override de organización** — las grants
///   organizacionales se incluyen siempre — más la pierna de sucursal, que solo
///   aporta cuando la sucursal existe en la organización, está ACTIVA, la
///   membership está ACTIVA y el rol de la membership está ACTIVO.
/// - Un permiso concedido por membership nunca se promociona a alcance de
///   organización: el scope de organización se resuelve con `branch_id = None`.
/// - Una sucursal inexistente o ajena no produce un error: no aporta permisos y
///   la clave foránea de tenant impide confundirla. No se revela su existencia.
pub trait PermissionAuthority {
    /// Devuelve las grants vigentes del usuario en el scope solicitado.
    fn resolve_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: Option<&EntityId>,
    ) -> impl Future<Output = Result<BTreeSet<PermissionCode>, IdentityError>> + Send;
    /// Variante de recuperación: igual que [`Self::resolve_permissions`] con
    /// `branch_id = Some(b)`, pero la pierna de sucursal no exige que la
    /// sucursal esté ACTIVA.
    ///
    /// Debe destinarse solo a flujos explícitamente autorizados sobre recursos
    /// ya abiertos; no es un bypass global ni desactiva las demás condiciones.
    fn resolve_permissions_for_recovery(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<BTreeSet<PermissionCode>, IdentityError>> + Send;
}

/// Coordinación de autenticación local sobre una única fuente DB-authoritative.
///
/// Identidad y permisos comparten la misma fuente para que la decisión no mezcle
/// orígenes con estados distintos. Para mutaciones críticas el llamador debe
/// construir este servicio sobre la conexión de la unidad de trabajo activa.
pub struct LocalPrincipalService<S> {
    source: S,
}
impl<S: LocalIdentityRepository + PermissionAuthority> LocalPrincipalService<S> {
    /// Ensambla la fuente explícita; no existe implementación allow-all.
    pub fn new(source: S) -> Self {
        Self { source }
    }
    /// Construye un principal para un scope normal y falla cerrado en cualquier duda.
    ///
    /// La organización del principal procede siempre de la sesión y del usuario,
    /// nunca de la petición.
    pub async fn authenticate(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
    ) -> Result<VerifiedPrincipal, AuditAttributionError> {
        self.authenticate_in(hash, now, branch_id, ScopeMode::Normal)
            .await
    }
    /// Construye un principal de recuperación para una sucursal concreta.
    ///
    /// La pierna de sucursal no exige sucursal ACTIVA; el resto de condiciones
    /// (usuario, organización, membership, rol) se mantiene. El principal
    /// resultante solo autoriza scopes en modo recuperación.
    pub async fn authenticate_recovery(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: &EntityId,
    ) -> Result<VerifiedPrincipal, AuditAttributionError> {
        self.authenticate_in(hash, now, Some(branch_id), ScopeMode::Recovery)
            .await
    }
    async fn authenticate_in(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
        mode: ScopeMode,
    ) -> Result<VerifiedPrincipal, AuditAttributionError> {
        if mode == ScopeMode::Recovery && branch_id.is_none() {
            return Err(AuditAttributionError::ScopeDenied);
        }
        let session = self
            .source
            .find_session_by_hash(hash)
            .await
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?
            .ok_or(AuditAttributionError::Unauthenticated)?;
        let user = self
            .source
            .find_user(session.organization_id(), session.user_id())
            .await
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?
            .ok_or(AuditAttributionError::Unauthenticated)?;
        session
            .ensure_owned_by(&user)
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?;
        session
            .validate_for(user.id(), user.organization_id(), now)
            .map_err(|error| match error {
                IdentityError::SessionNotActive => AuditAttributionError::Unauthenticated,
                _ => AuditAttributionError::UnattributablePrincipal,
            })?;
        if !user.is_active() {
            return Err(AuditAttributionError::Unauthenticated);
        }
        let permissions = match (mode, branch_id) {
            (ScopeMode::Recovery, Some(branch)) => {
                self.source
                    .resolve_permissions_for_recovery(user.id(), user.organization_id(), branch)
                    .await
            }
            _ => {
                self.source
                    .resolve_permissions(user.id(), user.organization_id(), branch_id)
                    .await
            }
        }
        .map_err(|_| AuditAttributionError::ScopeDenied)?;
        let scope = match mode {
            ScopeMode::Recovery => {
                let branch = branch_id.ok_or(AuditAttributionError::ScopeDenied)?;
                PrincipalScope::recovery(user.organization_id().clone(), branch.clone())
            }
            ScopeMode::Normal => {
                PrincipalScope::new(user.organization_id().clone(), branch_id.cloned())
            }
        };
        Ok(VerifiedPrincipal {
            user_id: user.id().clone(),
            session_id: session.id().clone(),
            scope,
            permissions,
        })
    }
}
