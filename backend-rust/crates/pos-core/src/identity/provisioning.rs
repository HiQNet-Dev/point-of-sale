//! Autorización de provisioning de una instalación (opción B del ADR-0025).
//!
//! El dominio no genera tokens ni conoce el canal de entrega: recibe el digest
//! SHA-256 ya calculado y decide si una presentación es válida para una
//! instalación, finalidad y ventana concretas. La emisión, la custodia y la
//! entrega pertenecen al emisor nativo; la activación comercial sigue pendiente
//! de aprobación y este módulo no habilita ningún comando.

use std::fmt;

use crate::organization_branch::EntityId;

/// Ventana mínima admitida para un token, en segundos.
pub const MIN_PROVISIONING_TTL_SECONDS: i64 = 60;
/// Ventana máxima admitida para un token, en segundos.
pub const MAX_PROVISIONING_TTL_SECONDS: i64 = 3600;
/// Longitud del digest SHA-256 en hexadecimal minúsculo.
pub const TOKEN_DIGEST_HEX_LEN: usize = 64;

/// Finalidad explícita del token; un token solo sirve para la suya.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningPurpose {
    /// Primer aprovisionamiento de una instalación sin identidad.
    Bootstrap,
    /// Sustitución administrativa de un derecho perdido o revocado.
    AdministrativeRecovery,
}

impl ProvisioningPurpose {
    /// Valor persistible en `bootstrap_authorization.purpose`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bootstrap => "BOOTSTRAP",
            Self::AdministrativeRecovery => "ADMINISTRATIVE_RECOVERY",
        }
    }

    /// Lee una finalidad conocida; no acepta valores nuevos.
    ///
    /// # Errors
    /// Devuelve [`ProvisioningError::InvalidPurpose`] para cualquier otro valor.
    pub fn parse(value: &str) -> Result<Self, ProvisioningError> {
        match value {
            "BOOTSTRAP" => Ok(Self::Bootstrap),
            "ADMINISTRATIVE_RECOVERY" => Ok(Self::AdministrativeRecovery),
            _ => Err(ProvisioningError::InvalidPurpose),
        }
    }
}

/// Estado observable de una autorización.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningState {
    /// Emitida y aún presentable.
    Pending,
    /// Consumida por un bootstrap confirmado.
    Consumed,
    /// Revocada administrativamente; no vuelve a `Pending`.
    Revoked,
}

/// Fallo de la autorización, sin incluir el token ni el digest presentado.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningError {
    /// Finalidad desconocida.
    InvalidPurpose,
    /// El digest no es SHA-256 hexadecimal minúsculo.
    InvalidDigest,
    /// La ventana de validez no está entre el mínimo y el máximo admitidos.
    InvalidWindow,
    /// El token ya caducó para el instante consultado.
    Expired,
    /// La autorización fue revocada.
    Revoked,
    /// La autorización ya fue consumida.
    AlreadyConsumed,
    /// El token pertenece a otra instalación.
    InstallationMismatch,
    /// El token se emitió para otra finalidad.
    PurposeMismatch,
    /// El digest presentado no corresponde.
    TokenMismatch,
}

impl fmt::Display for ProvisioningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPurpose => "finalidad de provisioning inválida",
            Self::InvalidDigest => "digest de token inválido",
            Self::InvalidWindow => "ventana de validez fuera de política",
            Self::Expired => "autorización expirada",
            Self::Revoked => "autorización revocada",
            Self::AlreadyConsumed => "autorización ya consumida",
            Self::InstallationMismatch => "el token pertenece a otra instalación",
            Self::PurposeMismatch => "el token se emitió para otra finalidad",
            Self::TokenMismatch => "token de provisioning no válido",
        })
    }
}

impl std::error::Error for ProvisioningError {}

/// Autorización de un token, ligada a una instalación y a una finalidad.
///
/// No conoce el token en claro: solo su digest. El consumo no se infiere de la
/// existencia de una organización; se marca en la misma unidad de trabajo que el
/// bootstrap.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisioningAuthorization {
    id: EntityId,
    installation_id: EntityId,
    purpose: ProvisioningPurpose,
    token_digest: String,
    issued_at: i64,
    expires_at: i64,
    consumed_at: Option<i64>,
    revoked_at: Option<i64>,
}

impl ProvisioningAuthorization {
    /// Emite una autorización nueva dentro de una ventana admitida.
    ///
    /// # Errors
    /// Devuelve [`ProvisioningError::InvalidDigest`] si el digest no es SHA-256
    /// hexadecimal y [`ProvisioningError::InvalidWindow`] si la vigencia queda
    /// fuera de [`MIN_PROVISIONING_TTL_SECONDS`]-[`MAX_PROVISIONING_TTL_SECONDS`].
    pub fn issue(
        id: EntityId,
        installation_id: EntityId,
        purpose: ProvisioningPurpose,
        token_digest: &str,
        issued_at: i64,
        expires_at: i64,
    ) -> Result<Self, ProvisioningError> {
        if !is_token_digest(token_digest) {
            return Err(ProvisioningError::InvalidDigest);
        }
        validate_window(issued_at, expires_at)?;
        Ok(Self {
            id,
            installation_id,
            purpose,
            token_digest: token_digest.to_owned(),
            issued_at,
            expires_at,
            consumed_at: None,
            revoked_at: None,
        })
    }

    /// Reconstruye una autorización persistida sin relajar sus invariantes.
    ///
    /// # Errors
    /// Devuelve el mismo error que [`ProvisioningAuthorization::issue`] más
    /// [`ProvisioningError::AlreadyConsumed`] si el consumo precede a la emisión.
    #[allow(clippy::too_many_arguments)]
    pub fn rehydrate(
        id: EntityId,
        installation_id: EntityId,
        purpose: ProvisioningPurpose,
        token_digest: &str,
        issued_at: i64,
        expires_at: i64,
        consumed_at: Option<i64>,
        revoked_at: Option<i64>,
    ) -> Result<Self, ProvisioningError> {
        let authorization = Self::issue(
            id,
            installation_id,
            purpose,
            token_digest,
            issued_at,
            expires_at,
        )?;
        if consumed_at.is_some_and(|value| value < issued_at)
            || revoked_at.is_some_and(|value| value < issued_at)
        {
            return Err(ProvisioningError::InvalidWindow);
        }
        Ok(Self {
            consumed_at,
            revoked_at,
            ..authorization
        })
    }

    /// Identidad de la autorización.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Instalación a la que está ligada.
    pub fn installation_id(&self) -> &EntityId {
        &self.installation_id
    }

    /// Finalidad declarada.
    pub const fn purpose(&self) -> ProvisioningPurpose {
        self.purpose
    }

    /// Digest persistible; nunca el token.
    pub fn token_digest(&self) -> &str {
        &self.token_digest
    }

    /// Instante de emisión, en segundos UTC.
    pub const fn issued_at(&self) -> i64 {
        self.issued_at
    }

    /// Instante de caducidad, en segundos UTC.
    pub const fn expires_at(&self) -> i64 {
        self.expires_at
    }

    /// Instante de consumo, si ocurrió.
    pub const fn consumed_at(&self) -> Option<i64> {
        self.consumed_at
    }

    /// Instante de revocación, si ocurrió.
    pub const fn revoked_at(&self) -> Option<i64> {
        self.revoked_at
    }

    /// Estado derivado de los instantes registrados.
    pub const fn state(&self) -> ProvisioningState {
        if self.revoked_at.is_some() {
            ProvisioningState::Revoked
        } else if self.consumed_at.is_some() {
            ProvisioningState::Consumed
        } else {
            ProvisioningState::Pending
        }
    }

    /// Comprueba si un token presentado autoriza esta instalación y finalidad.
    ///
    /// # Errors
    /// Devuelve el primer motivo aplicable: revocada, consumida, expirada,
    /// finalidad distinta, instalación distinta o digest distinto.
    pub fn authorize(
        &self,
        token_digest: &str,
        installation_id: &EntityId,
        purpose: ProvisioningPurpose,
        now: i64,
    ) -> Result<(), ProvisioningError> {
        if self.revoked_at.is_some() {
            return Err(ProvisioningError::Revoked);
        }
        if self.consumed_at.is_some() {
            return Err(ProvisioningError::AlreadyConsumed);
        }
        if now >= self.expires_at {
            return Err(ProvisioningError::Expired);
        }
        if purpose != self.purpose {
            return Err(ProvisioningError::PurposeMismatch);
        }
        if installation_id != &self.installation_id {
            return Err(ProvisioningError::InstallationMismatch);
        }
        if !digests_equal(&self.token_digest, token_digest) {
            return Err(ProvisioningError::TokenMismatch);
        }
        Ok(())
    }

    /// Consume la autorización si la presentación es válida.
    ///
    /// # Errors
    /// Propaga cualquier rechazo de [`ProvisioningAuthorization::authorize`] y
    /// deja el estado intacto.
    pub fn consume(
        &mut self,
        token_digest: &str,
        installation_id: &EntityId,
        purpose: ProvisioningPurpose,
        now: i64,
    ) -> Result<(), ProvisioningError> {
        self.authorize(token_digest, installation_id, purpose, now)?;
        self.consumed_at = Some(now);
        Ok(())
    }

    /// Revoca la autorización de forma idempotente.
    ///
    /// # Errors
    /// Devuelve [`ProvisioningError::AlreadyConsumed`] si ya se consumió; una
    /// revocación repetida no es un error.
    pub fn revoke(&mut self, now: i64) -> Result<(), ProvisioningError> {
        if self.consumed_at.is_some() {
            return Err(ProvisioningError::AlreadyConsumed);
        }
        if self.revoked_at.is_none() {
            self.revoked_at = Some(now);
        }
        Ok(())
    }
}

fn validate_window(issued_at: i64, expires_at: i64) -> Result<(), ProvisioningError> {
    let ttl = expires_at
        .checked_sub(issued_at)
        .ok_or(ProvisioningError::InvalidWindow)?;
    if !(MIN_PROVISIONING_TTL_SECONDS..=MAX_PROVISIONING_TTL_SECONDS).contains(&ttl) {
        return Err(ProvisioningError::InvalidWindow);
    }
    Ok(())
}

fn is_token_digest(value: &str) -> bool {
    value.len() == TOKEN_DIGEST_HEX_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Compara digests sin cortocircuito por diferencia.
fn digests_equal(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}
