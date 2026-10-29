//! Transición de credenciales **de laboratorio**: verificación BCrypt y rehash
//! Argon2id con sección crítica mínima.
//!
//! # Flujo
//!
//! ```text
//! snapshot (lectura corta)
//!   -> verificación criptográfica fuera de la transacción (spawn_blocking)
//!   -> BEGIN IMMEDIATE
//!        relectura + validación del snapshot
//!        mutación con CAS sobre credential_version
//!        auditoría requerida
//!      COMMIT
//! ```
//!
//! Ninguna transición se anuncia como persistida si el CAS no confirmó la fila.
//! No es autenticación productiva: no emite sesiones ni entrega handles.

use std::{future::Future, sync::Arc};

use pos_core::{
    identity::{
        CredentialAlgorithm, CredentialKind, CredentialParameters, CredentialState, IdentityError,
        LocalCredential, LockoutPolicy, SecretHash, UtcSeconds,
    },
    organization_branch::EntityId,
};
use pos_credentials::{
    Argon2Policy, VerificationOutcome, VerifyRequest, hash_password, verify_credential,
};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork};

/// Resultado interno de un intento de credencial.
///
/// Distingue los casos para el caso de uso; una futura API debe colapsarlos en un
/// error genérico para no facilitar enumeración de usuarios.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialTransition {
    /// Secreto correcto; `rehash_performed` indica migración a Argon2id.
    Verified {
        /// `true` si el material se sustituyó por Argon2id y el CAS lo confirmó.
        rehash_performed: bool,
    },
    /// Secreto incorrecto; puede haber incrementado el contador de fallos.
    Rejected,
    /// Credencial bloqueada temporalmente.
    Locked,
    /// Material no verificable: exige reset, sin fallback.
    ResetRequired,
}

/// Datos de un intento de credencial.
#[derive(Clone)]
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

impl std::fmt::Debug for CredentialAttempt<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CredentialAttempt")
            .field("organization_id", &self.organization_id)
            .field("user_id", &self.user_id)
            .field("kind", &self.kind)
            .field("now", &self.now)
            .field("target_policy", &self.target_policy)
            .field("lockout", &self.lockout)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Evento de auditoría que acompaña a una mutación realmente persistida.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialAuditEntry {
    /// Organización del usuario.
    pub organization_id: EntityId,
    /// Usuario afectado.
    pub user_id: EntityId,
    /// Transición persistida.
    pub transition: CredentialTransition,
    /// Instante UTC de la operación.
    pub occurred_at: UtcSeconds,
}

/// Writer de auditoría que participa en la transacción de la credencial.
///
/// Solo se invoca cuando una mutación fue confirmada por CAS. Un error aquí debe
/// provocar el rollback completo de la operación.
pub trait CredentialAuditWriter {
    /// Inserta el evento y devuelve error para forzar el rollback.
    fn append(
        &self,
        transaction: &mut SqliteTransaction,
        entry: &CredentialAuditEntry,
    ) -> impl Future<Output = Result<(), IdentityError>> + Send;
}

/// Decisión criptográfica preparada fuera de la transacción.
pub struct PreparedCredential {
    decision: Decision,
    fingerprint: MaterialFingerprint,
}

/// Identidad del material sobre el que se tomó la decisión.
///
/// No deriva `Debug` para no exponer el hash en diagnósticos.
#[derive(Clone, Eq, PartialEq)]
struct MaterialFingerprint {
    algorithm: CredentialAlgorithm,
    policy_id: String,
    parameters: String,
    secret_hash: String,
}

impl MaterialFingerprint {
    fn of(credential: &LocalCredential) -> Self {
        Self {
            algorithm: credential.algorithm(),
            policy_id: credential.policy_id().to_owned(),
            parameters: credential.parameters().canonical_json().to_owned(),
            secret_hash: credential.secret_hash().expose_to_verifier().to_owned(),
        }
    }
}

enum Decision {
    Verified { rehash: Option<PreparedMaterial> },
    Rejected,
    Unusable,
}

struct PreparedMaterial {
    secret_hash: SecretHash,
    parameters: CredentialParameters,
    policy_id: &'static str,
}

impl std::fmt::Debug for PreparedCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PreparedCredential(REDACTED)")
    }
}

/// Aplica la verificación criptográfica y decide la mutación pendiente.
///
/// Es una función de CPU: el llamador debe ejecutarla fuera de la transacción de
/// escritura (por ejemplo con `spawn_blocking`).
///
/// # Errors
/// Devuelve [`IdentityError::StorageUnavailable`] si no se pudo preparar el
/// material de rehash.
pub fn prepare(
    credential: &LocalCredential,
    kind: CredentialKind,
    password: &str,
    target_policy: Argon2Policy,
) -> Result<PreparedCredential, IdentityError> {
    let outcome = {
        let request = VerifyRequest {
            purpose: kind,
            algorithm: credential.algorithm(),
            stored: credential.secret_hash(),
            password,
        };
        verify_credential(&request)
    };
    let decision = match outcome {
        Ok(VerificationOutcome::Verified {
            rehash_required: false,
        }) => Decision::Verified { rehash: None },
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
            Decision::Verified {
                rehash: Some(PreparedMaterial {
                    secret_hash,
                    parameters,
                    policy_id: target_policy.id(),
                }),
            }
        }
        Ok(VerificationOutcome::Rejected) => Decision::Rejected,
        Ok(VerificationOutcome::Unusable) => Decision::Unusable,
        // Entrada inválida: no se toca el estado almacenado.
        Err(_) => Decision::Rejected,
    };
    Ok(PreparedCredential {
        decision,
        fingerprint: MaterialFingerprint::of(credential),
    })
}

/// Resultado de aplicar una credencial ya preparada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppliedCredential {
    /// Transición observada.
    pub transition: CredentialTransition,
    /// `true` solo si el CAS confirmó la fila modificada.
    pub persisted: bool,
}

/// Adaptador de credenciales ligado a una conexión SQLite concreta.
pub struct SqliteCredentialLab<'a> {
    connection: &'a mut SqliteConnection,
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

    /// Aplica la decisión preparada dentro de la transacción actual.
    ///
    /// Relee el estado vigente y exige que la versión no haya cambiado para las
    /// decisiones `Verified`: una verificación criptográfica no se reutiliza
    /// sobre material distinto. Un usuario desactivado entre el snapshot y el
    /// commit se rechaza sin mutación.
    pub async fn apply(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
        kind: CredentialKind,
        prepared: PreparedCredential,
        now: UtcSeconds,
        lockout: LockoutPolicy,
    ) -> Result<AppliedCredential, IdentityError> {
        let active_user = sqlx::query_scalar::<_, String>(
            "SELECT status FROM identity_user_lab WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .fetch_optional(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        if active_user.as_deref() != Some("ACTIVE") {
            return Ok(AppliedCredential {
                transition: CredentialTransition::Rejected,
                persisted: false,
            });
        }
        let Some(mut current) = self.load(organization_id, user_id, kind).await? else {
            return Err(IdentityError::ConcurrentModification);
        };
        match current.state() {
            CredentialState::Active => {}
            CredentialState::ResetRequired => {
                return Ok(AppliedCredential {
                    transition: CredentialTransition::ResetRequired,
                    persisted: false,
                });
            }
            CredentialState::Disabled => {
                return Ok(AppliedCredential {
                    transition: CredentialTransition::Rejected,
                    persisted: false,
                });
            }
        }
        if current.is_locked(now) {
            return Ok(AppliedCredential {
                transition: CredentialTransition::Locked,
                persisted: false,
            });
        }
        let version = current.credential_version();
        if MaterialFingerprint::of(&current) != prepared.fingerprint {
            // El material cambió después del snapshot: la decisión criptográfica
            // no se reutiliza, aunque solo haya cambiado el estado de la fila.
            return Err(IdentityError::ConcurrentModification);
        }
        let (transition, mutated) = match prepared.decision {
            Decision::Verified { rehash } => {
                if let Some(material) = rehash {
                    current.rotate_material(
                        CredentialAlgorithm::Argon2id,
                        material.secret_hash,
                        material.parameters,
                        material.policy_id,
                        now,
                        version,
                    )?;
                    (
                        CredentialTransition::Verified {
                            rehash_performed: true,
                        },
                        true,
                    )
                } else {
                    let cleared = current.record_successful_login(now, version)?;
                    (
                        CredentialTransition::Verified {
                            rehash_performed: false,
                        },
                        cleared,
                    )
                }
            }
            Decision::Rejected => {
                let locked = current.record_failed_login(now, lockout, version)?;
                (
                    if locked {
                        CredentialTransition::Locked
                    } else {
                        CredentialTransition::Rejected
                    },
                    true,
                )
            }
            Decision::Unusable => {
                current.require_reset(version)?;
                (CredentialTransition::ResetRequired, true)
            }
        };
        if !mutated {
            return Ok(AppliedCredential {
                transition,
                persisted: false,
            });
        }
        match self.persist(organization_id, version, &current).await? {
            CasOutcome::Updated => Ok(AppliedCredential {
                transition,
                persisted: true,
            }),
            // Un conflicto nunca confirma la mutación parcial ni el resultado.
            CasOutcome::Missing | CasOutcome::Conflict => {
                Err(IdentityError::ConcurrentModification)
            }
        }
    }

    async fn persist(
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
}

/// Límite de verificaciones criptográficas concurrentes.
///
/// El permiso se mueve al trabajo bloqueante: cancelar el future que espera no
/// libera el slot mientras el cálculo siga en ejecución, y nunca se retiene un
/// lock de SQLite esperando capacidad.
#[derive(Clone)]
pub struct VerificationGate {
    permits: Arc<Semaphore>,
}

impl VerificationGate {
    /// Crea un límite explícito de verificaciones concurrentes.
    ///
    /// # Errors
    /// Devuelve [`IdentityError::StorageUnavailable`] si el límite es cero.
    pub fn new(max_concurrent: usize) -> Result<Self, IdentityError> {
        if max_concurrent == 0 {
            return Err(IdentityError::StorageUnavailable);
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(max_concurrent)),
        })
    }

    /// Slots libres; útil para pruebas y telemetría acotada.
    pub fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }

    async fn run<T, F>(&self, work: F) -> Result<T, IdentityError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| IdentityError::StorageUnavailable)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .map_err(|_| IdentityError::StorageUnavailable)
    }
}

/// Verifica una credencial reduciendo la sección crítica de escritura.
///
/// 1. Snapshot y validación de estado fuera de la transacción.
/// 2. Criptografía en `spawn_blocking` bajo un límite de concurrencia.
/// 3. `BEGIN IMMEDIATE`, relectura, validación del material, CAS y auditoría.
///
/// # Errors
/// Devuelve [`IdentityError::ConcurrentModification`] si el material o el estado
/// cambiaron entre el snapshot y la aplicación, y
/// [`IdentityError::StorageUnavailable`] ante fallos de almacenamiento, commit o
/// criptografía interna.
pub async fn authenticate_and_upgrade<W: CredentialAuditWriter>(
    pool: &SqlitePool,
    unit_of_work: &SqliteUnitOfWork,
    attempt: CredentialAttempt<'_>,
    audit: &W,
    gate: &VerificationGate,
) -> Result<CredentialTransition, IdentityError> {
    let snapshot = {
        let mut connection = pool
            .acquire()
            .await
            .map_err(|_| IdentityError::StorageUnavailable)?;
        SqliteCredentialLab::new(&mut connection)
            .load(&attempt.organization_id, &attempt.user_id, attempt.kind)
            .await?
    };
    let Some(credential) = snapshot else {
        return Ok(CredentialTransition::Rejected);
    };
    match credential.state() {
        CredentialState::Active => {}
        CredentialState::ResetRequired => return Ok(CredentialTransition::ResetRequired),
        CredentialState::Disabled => return Ok(CredentialTransition::Rejected),
    }
    if credential.is_locked(attempt.now) {
        return Ok(CredentialTransition::Locked);
    }

    let kind = attempt.kind;
    // La copia temporal se borra al salir del alcance; el original nunca se clona
    // fuera de esta frontera ni se registra.
    let password = Zeroizing::new(attempt.password.to_owned());
    let target_policy = attempt.target_policy;
    let prepared = gate
        .run(move || prepare(&credential, kind, password.as_str(), target_policy))
        .await??;

    let mut transaction = unit_of_work
        .begin()
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
    let applied = {
        let mut lab = SqliteCredentialLab::new(transaction.connection());
        lab.apply(
            &attempt.organization_id,
            &attempt.user_id,
            attempt.kind,
            prepared,
            attempt.now,
            attempt.lockout,
        )
        .await
    };
    let applied = match applied {
        Ok(applied) => applied,
        Err(error) => {
            transaction
                .rollback()
                .await
                .map_err(|_| IdentityError::StorageUnavailable)?;
            return Err(error);
        }
    };
    if applied.persisted {
        let entry = CredentialAuditEntry {
            organization_id: attempt.organization_id.clone(),
            user_id: attempt.user_id.clone(),
            transition: applied.transition,
            occurred_at: attempt.now,
        };
        if audit.append(&mut transaction, &entry).await.is_err() {
            transaction
                .rollback()
                .await
                .map_err(|_| IdentityError::StorageUnavailable)?;
            return Err(IdentityError::StorageUnavailable);
        }
    }
    transaction
        .commit()
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
    Ok(applied.transition)
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
