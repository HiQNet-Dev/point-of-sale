//! Transición de credenciales **de laboratorio**: verificación BCrypt y rehash
//! Argon2id sobre la conexión de una unidad de trabajo.
//!
//! No es autenticación productiva: no emite sesiones, no entrega handles y no
//! registra auditoría por sí mismo. El llamador es responsable de insertar el
//! evento de auditoría en la **misma** conexión y de confirmar un solo commit.

use pos_core::{
    identity::{
        CredentialAlgorithm, CredentialKind, CredentialParameters, CredentialState, IdentityError,
        LocalCredential, LockoutPolicy, SecretHash, UtcSeconds, VerifiedPrincipal,
    },
    organization_branch::EntityId,
};
use pos_credentials::{
    Argon2Policy, VerificationOutcome, VerifyRequest, hash_password, verify_credential,
};
use sqlx::{FromRow, SqliteConnection};

use crate::CasOutcome;

/// Resultado interno de un intento de credencial.
///
/// Distingue los casos para el caso de uso; una futura API debe colapsarlos en un
/// error genérico para no facilitar enumeración de usuarios.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialTransition {
    /// Secreto correcto; `rehash_performed` indica migración a Argon2id.
    Verified {
        /// `true` si el material se sustituyó por Argon2id.
        rehash_performed: bool,
    },
    /// Secreto incorrecto; puede haber incrementado el contador de fallos.
    Rejected,
    /// Credencial bloqueada temporalmente.
    Locked,
    /// Material no verificable: exige reset, sin fallback.
    ResetRequired,
}

/// Adaptador de credenciales ligado a una conexión SQLite concreta.
pub struct SqliteCredentialLab<'a> {
    connection: &'a mut SqliteConnection,
}

/// Datos de un intento de credencial sobre una conexión concreta.
#[derive(Clone, Debug)]
pub struct CredentialAttempt<'a> {
    /// Organización del usuario; nunca se toma del cliente no verificado.
    pub organization_id: EntityId,
    /// Usuario cuya credencial se verifica.
    pub user_id: EntityId,
    /// Finalidad solicitada.
    pub kind: CredentialKind,
    /// Secreto presentado; nunca se registra.
    pub password: &'a str,
    /// Instante UTC del intento.
    pub now: UtcSeconds,
    /// Política destino de rehash.
    pub target_policy: Argon2Policy,
    /// Política de bloqueo inyectada.
    pub lockout: LockoutPolicy,
}

impl<'a> SqliteCredentialLab<'a> {
    /// Enlaza el adaptador a una conexión; puede ser la de una UoW activa.
    pub fn new(connection: &'a mut SqliteConnection) -> Self {
        Self { connection }
    }

    /// Carga la credencial de un usuario para una finalidad concreta.
    pub async fn load(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
        kind: CredentialKind,
    ) -> Result<Option<LocalCredential>, IdentityError> {
        let row = sqlx::query_as::<_, CredentialRow>(
            "SELECT id, user_id, credential_kind, algorithm, secret_hash, \
                    params_json, policy_id, state, credential_version, failed_login_attempts, \
                    locked_until, created_at, updated_at \
             FROM identity_credential_lab \
             WHERE organization_id = ? AND user_id = ? AND credential_kind = ?",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .bind(kind_code(kind))
        .fetch_optional(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        row.map(CredentialRow::into_domain).transpose()
    }

    /// Persiste la credencial con CAS sobre `credential_version`.
    pub async fn persist(
        &mut self,
        organization_id: &EntityId,
        previous_version: u64,
        credential: &LocalCredential,
    ) -> Result<CasOutcome, IdentityError> {
        let expected =
            i64::try_from(previous_version).map_err(|_| IdentityError::StorageUnavailable)?;
        let version = i64::try_from(credential.credential_version())
            .map_err(|_| IdentityError::StorageUnavailable)?;
        let attempts = i64::from(credential.failed_login_attempts());
        let result = sqlx::query(
            "UPDATE identity_credential_lab \
             SET algorithm = ?, secret_hash = ?, params_json = ?, policy_id = ?, state = ?, \
                 credential_version = ?, failed_login_attempts = ?, locked_until = ?, updated_at = ? \
             WHERE organization_id = ? AND user_id = ? AND credential_kind = ? \
               AND credential_version = ?",
        )
        .bind(algorithm_code(credential.algorithm()))
        .bind(credential.secret_hash().expose_to_verifier())
        .bind(credential.parameters().canonical_json())
        .bind(credential.policy_id())
        .bind(state_code(credential.state()))
        .bind(version)
        .bind(attempts)
        .bind(credential.locked_until().map(UtcSeconds::get))
        .bind(credential.updated_at().get())
        .bind(organization_id.as_str())
        .bind(credential.user_id().as_str())
        .bind(kind_code(credential.kind()))
        .bind(expected)
        .execute(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        Ok(if result.rows_affected() == 1 {
            CasOutcome::Updated
        } else {
            CasOutcome::Conflict
        })
    }

    /// Verifica un secreto y aplica la transición BCrypt → Argon2id con CAS.
    ///
    /// No abre transacción: el llamador debe invocarlo dentro de la UoW de la
    /// operación para que mutación y auditoría compartan commit.
    pub async fn verify_and_upgrade(
        &mut self,
        attempt: CredentialAttempt<'_>,
    ) -> Result<CredentialTransition, IdentityError> {
        let CredentialAttempt {
            organization_id,
            user_id,
            kind,
            password,
            now,
            target_policy,
            lockout,
        } = attempt;
        let Some(mut credential) = self.load(&organization_id, &user_id, kind).await? else {
            return Ok(CredentialTransition::Rejected);
        };
        match credential.state() {
            CredentialState::Active => {}
            CredentialState::ResetRequired => return Ok(CredentialTransition::ResetRequired),
            CredentialState::Disabled => return Ok(CredentialTransition::Rejected),
        }
        if credential.is_locked(now) {
            return Ok(CredentialTransition::Locked);
        }
        let version = credential.credential_version();
        let outcome = {
            let request = VerifyRequest {
                purpose: kind,
                algorithm: credential.algorithm(),
                stored: credential.secret_hash(),
                password,
            };
            verify_credential(&request)
        };
        match outcome {
            Ok(VerificationOutcome::Verified {
                rehash_required: false,
            }) => Ok(CredentialTransition::Verified {
                rehash_performed: false,
            }),
            Ok(VerificationOutcome::Verified {
                rehash_required: true,
            }) => {
                let secret_hash = hash_password(password, target_policy)
                    .map_err(|_| IdentityError::StorageUnavailable)?;
                let parameters = CredentialParameters::new(&serde_json::json!({
                    "m_cost": target_policy.memory_kib(),
                    "t_cost": target_policy.iterations(),
                    "p_cost": target_policy.lanes(),
                }))
                .map_err(|_| IdentityError::StorageUnavailable)?;
                credential.rotate_material(
                    CredentialAlgorithm::Argon2id,
                    secret_hash,
                    parameters,
                    target_policy.id(),
                    now,
                    version,
                )?;
                self.persist(&organization_id, version, &credential).await?;
                Ok(CredentialTransition::Verified {
                    rehash_performed: true,
                })
            }
            Ok(VerificationOutcome::Rejected) => {
                let locked = credential.record_failed_login(now, lockout, version)?;
                self.persist(&organization_id, version, &credential).await?;
                Ok(if locked {
                    CredentialTransition::Locked
                } else {
                    CredentialTransition::Rejected
                })
            }
            Ok(VerificationOutcome::Unusable) => {
                credential.require_reset(version)?;
                self.persist(&organization_id, version, &credential).await?;
                Ok(CredentialTransition::ResetRequired)
            }
            // Entrada inválida: no se toca el estado almacenado.
            Err(_) => Ok(CredentialTransition::Rejected),
        }
    }

    /// Variante scoped a un principal autenticado para credenciales PASSWORD.
    ///
    /// El usuario y la organización proceden del principal; conocer un UUID ajeno
    /// no habilita la operación.
    pub async fn verify_and_upgrade_for(
        &mut self,
        principal: &VerifiedPrincipal,
        password: &str,
        now: UtcSeconds,
        target_policy: Argon2Policy,
        lockout: LockoutPolicy,
    ) -> Result<CredentialTransition, IdentityError> {
        self.verify_and_upgrade(CredentialAttempt {
            organization_id: principal.scope().organization_id().clone(),
            user_id: principal.user_id().clone(),
            kind: CredentialKind::Password,
            password,
            now,
            target_policy,
            lockout,
        })
        .await
    }
}

fn kind_code(kind: CredentialKind) -> &'static str {
    match kind {
        CredentialKind::Password => "PASSWORD",
        CredentialKind::Pin => "PIN",
    }
}

fn algorithm_code(algorithm: CredentialAlgorithm) -> &'static str {
    match algorithm {
        CredentialAlgorithm::LegacyBcrypt => "LEGACY_BCRYPT",
        CredentialAlgorithm::Argon2id => "ARGON2ID",
    }
}

fn state_code(state: CredentialState) -> &'static str {
    match state {
        CredentialState::Active => "ACTIVE",
        CredentialState::ResetRequired => "RESET_REQUIRED",
        CredentialState::Disabled => "DISABLED",
    }
}

#[derive(FromRow)]
struct CredentialRow {
    id: String,
    user_id: String,
    credential_kind: String,
    algorithm: String,
    secret_hash: String,
    params_json: String,
    policy_id: String,
    state: String,
    credential_version: i64,
    failed_login_attempts: i64,
    locked_until: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

impl CredentialRow {
    fn into_domain(self) -> Result<LocalCredential, IdentityError> {
        let kind = match self.credential_kind.as_str() {
            "PASSWORD" => CredentialKind::Password,
            "PIN" => CredentialKind::Pin,
            _ => return Err(IdentityError::StorageUnavailable),
        };
        let algorithm = match self.algorithm.as_str() {
            "LEGACY_BCRYPT" => CredentialAlgorithm::LegacyBcrypt,
            "ARGON2ID" => CredentialAlgorithm::Argon2id,
            _ => return Err(IdentityError::StorageUnavailable),
        };
        let state = match self.state.as_str() {
            "ACTIVE" => CredentialState::Active,
            "RESET_REQUIRED" => CredentialState::ResetRequired,
            "DISABLED" => CredentialState::Disabled,
            _ => return Err(IdentityError::StorageUnavailable),
        };
        let parsed: serde_json::Value = serde_json::from_str(&self.params_json)
            .map_err(|_| IdentityError::StorageUnavailable)?;
        let parameters =
            CredentialParameters::new(&parsed).map_err(|_| IdentityError::StorageUnavailable)?;
        let secret_hash =
            SecretHash::new(&self.secret_hash).map_err(|_| IdentityError::StorageUnavailable)?;
        let attempts = u32::try_from(self.failed_login_attempts)
            .map_err(|_| IdentityError::StorageUnavailable)?;
        LocalCredential::restore(
            EntityId::parse(&self.id).map_err(|_| IdentityError::StorageUnavailable)?,
            EntityId::parse(&self.user_id).map_err(|_| IdentityError::StorageUnavailable)?,
            kind,
            algorithm,
            secret_hash,
            parameters,
            &self.policy_id,
            state,
            u64::try_from(self.credential_version)
                .map_err(|_| IdentityError::StorageUnavailable)?,
            attempts,
            self.locked_until.map(UtcSeconds::new).transpose()?,
            UtcSeconds::new(self.created_at)?,
            UtcSeconds::new(self.updated_at)?,
        )
        .map_err(|_| IdentityError::StorageUnavailable)
    }
}
