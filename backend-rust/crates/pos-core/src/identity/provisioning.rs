//! Autorización de provisioning de una instalación (opción B del ADR-0025).
//!
//! El dominio no genera tokens ni conoce el canal de entrega: recibe el digest
//! SHA-256 ya calculado y decide si una presentación es válida para una
//! instalación, finalidad y ventana concretas. La emisión, la custodia y la
//! entrega pertenecen al emisor nativo; la activación comercial sigue pendiente
//! de aprobación y este módulo no habilita ningún comando.
//!
//! El identificador de instalación es el de la **preautorización**: existe antes
//! de que haya organización y se vincula a la instalación canónica al completar
//! el bootstrap. No se crea una organización ficticia para emitir un token.

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

/// Estado de una autorización; los no pendientes son terminales.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningState {
    /// Emitida y aún presentable, mientras no caduque.
    Pending,
    /// Consumida por un bootstrap confirmado.
    Consumed,
    /// Revocada administrativamente.
    Revoked,
    /// Caducada sin consumo; es terminal y no vuelve a `Pending`.
    Expired,
}

impl ProvisioningState {
    /// Valor persistible en `bootstrap_authorization.state`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Consumed => "CONSUMED",
            Self::Revoked => "REVOKED",
            Self::Expired => "EXPIRED",
        }
    }

    /// Indica si el estado ya no admite presentación.
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending)
    }

    /// Lee un estado persistido conocido.
    ///
    /// # Errors
    /// Devuelve [`ProvisioningError::IncoherentState`] para valores ajenos.
    pub fn parse(value: &str) -> Result<Self, ProvisioningError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "CONSUMED" => Ok(Self::Consumed),
            "REVOKED" => Ok(Self::Revoked),
            "EXPIRED" => Ok(Self::Expired),
            _ => Err(ProvisioningError::IncoherentState),
        }
    }
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
    /// El reloj consultado es anterior a la emisión.
    ClockBeforeIssue,
    /// Estado y cierre persistidos no son coherentes.
    IncoherentState,
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
            Self::ClockBeforeIssue => "el reloj es anterior a la emisión",
            Self::IncoherentState => "estado de autorización incoherente",
        })
    }
}

impl std::error::Error for ProvisioningError {}

/// Autorización de un token, ligada a una instalación y a una finalidad.
///
/// No conoce el token en claro: solo su digest. El consumo se confirma en la
/// misma transacción que el bootstrap; un rechazo no muta el agregado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisioningAuthorization {
    id: EntityId,
    installation_id: EntityId,
    purpose: ProvisioningPurpose,
    token_digest: String,
    issued_at: i64,
    expires_at: i64,
    state: ProvisioningState,
    closed_at: Option<i64>,
}

impl ProvisioningAuthorization {
    /// Calcula la caducidad de una emisión, rechazando desbordamiento.
    ///
    /// # Errors
    /// Devuelve [`ProvisioningError::InvalidWindow`] si la duración queda fuera
    /// de política o si `issued_at` + `ttl` desborda `i64`.
    pub fn expires_from(issued_at: i64, ttl_seconds: i64) -> Result<i64, ProvisioningError> {
        if !(MIN_PROVISIONING_TTL_SECONDS..=MAX_PROVISIONING_TTL_SECONDS).contains(&ttl_seconds) {
            return Err(ProvisioningError::InvalidWindow);
        }
        issued_at
            .checked_add(ttl_seconds)
            .ok_or(ProvisioningError::InvalidWindow)
    }

    /// Emite una autorización pendiente dentro de una ventana admitida.
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
            state: ProvisioningState::Pending,
            closed_at: None,
        })
    }

    /// Reconstruye una autorización persistida sin relajar sus invariantes.
    ///
    /// # Errors
    /// Devuelve [`ProvisioningError::IncoherentState`] si el estado y el cierre
    /// no concuerdan, y los mismos errores que [`ProvisioningAuthorization::issue`].
    #[allow(clippy::too_many_arguments)]
    pub fn rehydrate(
        id: EntityId,
        installation_id: EntityId,
        purpose: ProvisioningPurpose,
        token_digest: &str,
        issued_at: i64,
        expires_at: i64,
        state: ProvisioningState,
        closed_at: Option<i64>,
    ) -> Result<Self, ProvisioningError> {
        let authorization = Self::issue(
            id,
            installation_id,
            purpose,
            token_digest,
            issued_at,
            expires_at,
        )?;
        match (state, closed_at) {
            (ProvisioningState::Pending, None) => {}
            (ProvisioningState::Pending, Some(_)) => {
                return Err(ProvisioningError::IncoherentState);
            }
            (_, None) => return Err(ProvisioningError::IncoherentState),
            (_, Some(closed)) if closed < issued_at => {
                return Err(ProvisioningError::IncoherentState);
            }
            (ProvisioningState::Expired, Some(closed)) if closed < expires_at => {
                return Err(ProvisioningError::IncoherentState);
            }
            _ => {}
        }
        Ok(Self {
            state,
            closed_at,
            ..authorization
        })
    }

    /// Identidad de la autorización.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Instalación preautorizada a la que está ligada.
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

    /// Estado persistido.
    pub const fn state(&self) -> ProvisioningState {
        self.state
    }

    /// Estado efectivo para un instante dado; la caducidad es perezosa.
    pub const fn state_at(&self, now: i64) -> ProvisioningState {
        if self.state.is_terminal() {
            self.state
        } else if now >= self.expires_at {
            ProvisioningState::Expired
        } else {
            ProvisioningState::Pending
        }
    }

    /// Indica si seguía presentable en `now`.
    pub const fn is_live_at(&self, now: i64) -> bool {
        matches!(self.state_at(now), ProvisioningState::Pending)
    }

    /// Instante de cierre (`consumed_at`, `revoked_at` o caducidad), si lo hubo.
    pub const fn closed_at(&self) -> Option<i64> {
        self.closed_at
    }

    /// Comprueba si un token presentado autoriza esta instalación y finalidad.
    ///
    /// # Errors
    /// Devuelve el primer motivo aplicable: reloj anterior a la emisión,
    /// autorización no pendiente, caducidad, finalidad distinta, instalación
    /// distinta o digest distinto.
    pub fn authorize(
        &self,
        token_digest: &str,
        installation_id: &EntityId,
        purpose: ProvisioningPurpose,
        now: i64,
    ) -> Result<(), ProvisioningError> {
        if now < self.issued_at {
            return Err(ProvisioningError::ClockBeforeIssue);
        }
        match self.state {
            ProvisioningState::Revoked => return Err(ProvisioningError::Revoked),
            ProvisioningState::Consumed => return Err(ProvisioningError::AlreadyConsumed),
            ProvisioningState::Expired => return Err(ProvisioningError::Expired),
            ProvisioningState::Pending => {}
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
        self.state = ProvisioningState::Consumed;
        self.closed_at = Some(now);
        Ok(())
    }

    /// Revoca una autorización pendiente; repetirla no es un error.
    ///
    /// Una autorización ya caducada permanece `Expired`: la revocación no revive
    /// ni reescribe ese estado terminal.
    ///
    /// # Errors
    /// Devuelve [`ProvisioningError::AlreadyConsumed`] si ya se consumió.
    pub fn revoke(&mut self, now: i64) -> Result<(), ProvisioningError> {
        match self.state {
            ProvisioningState::Consumed => Err(ProvisioningError::AlreadyConsumed),
            ProvisioningState::Revoked | ProvisioningState::Expired => Ok(()),
            ProvisioningState::Pending => {
                if now < self.issued_at {
                    return Err(ProvisioningError::ClockBeforeIssue);
                }
                self.state = ProvisioningState::Revoked;
                self.closed_at = Some(now);
                Ok(())
            }
        }
    }

    /// Marca como `Expired` una autorización pendiente cuya ventana terminó.
    ///
    /// Devuelve `true` si cambió el estado. El cierre se fija en `expires_at`
    /// para que la transición sea determinista y auditable.
    pub fn expire(&mut self, now: i64) -> bool {
        if self.state == ProvisioningState::Pending && now >= self.expires_at {
            self.state = ProvisioningState::Expired;
            self.closed_at = Some(self.expires_at);
            return true;
        }
        false
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
