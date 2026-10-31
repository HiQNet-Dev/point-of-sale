use crate::organization_branch::EntityId;
use serde_json::{Map, Value};
use std::fmt;

use super::*;

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
