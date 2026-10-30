use crate::organization_branch::EntityId;
use std::fmt;

use super::*;

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
