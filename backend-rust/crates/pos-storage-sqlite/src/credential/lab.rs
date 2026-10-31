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

use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

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
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
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
/// No guarda el hash codificado: solo su digest SHA-256, para que ni `Debug` ni
/// un volcado accidental del valor preparado expongan material reutilizable.
/// El fingerprint no deriva `Debug`.
#[derive(Clone, Eq, PartialEq)]
struct MaterialFingerprint {
    algorithm: CredentialAlgorithm,
    policy_id: String,
    parameters: String,
    hash_digest: [u8; 32],
}

impl MaterialFingerprint {
    fn of(credential: &LocalCredential) -> Self {
        Self {
            algorithm: credential.algorithm(),
            policy_id: credential.policy_id().to_owned(),
            parameters: credential.parameters().canonical_json().to_owned(),
            hash_digest: digest(credential.secret_hash().expose_to_verifier()),
        }
    }
}

fn digest(value: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let output = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&output);
    bytes
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
    queries: &'static super::queries::CredentialQueries,
}

impl<'a> SqliteCredentialLab<'a> {
    /// Enlaza el adaptador a una conexión; puede ser la de una UoW activa.
    pub fn new(connection: &'a mut SqliteConnection) -> Self {
        Self {
            connection,
            queries: &super::queries::LAB,
        }
    }

    /// Enlaza la misma implementación con local_credential/pos_user candidatos.
    /// No migra ni activa este esquema; conserva sus timestamps y catálogo físico.
    pub fn canonical(connection: &'a mut SqliteConnection) -> Self {
        Self {
            connection,
            queries: &super::queries::CANONICAL,
        }
    }

    /// Carga la credencial de un usuario para una finalidad concreta.
    pub async fn load(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
        kind: CredentialKind,
    ) -> Result<Option<LocalCredential>, IdentityError> {
        let row = sqlx::query_as::<_, CredentialRow>(self.queries.load)
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
        let active_user = sqlx::query_scalar::<_, String>(self.queries.active_user)
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
        let result = sqlx::query(self.queries.persist)
            .bind(algorithm_code(credential.algorithm()))
            .bind(credential.secret_hash().expose_to_verifier())
            .bind(credential.parameters().canonical_json())
            .bind(credential.policy_id())
            .bind(match credential.state() {
                CredentialState::ResetRequired => self.queries.reset_state,
                state => state_code(state),
            })
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
///
/// Una verificación Argon2id con `m=19456 KiB` reserva ~19 MiB por operación,
/// pero el asignador retiene bloques por hilo: ocho verificaciones simultáneas
/// midieron ~1.4 GiB de RSS en un equipo de 2 vCPU (ver
/// `docs/05-development/rust-auth-benchmark-2a.md`). Por eso el límite no es solo
/// un semáforo: admite una cola acotada y un tiempo máximo de espera.
#[derive(Clone)]
pub struct VerificationGate {
    permits: Arc<Semaphore>,
    admission: Option<Arc<Admission>>,
    max_wait: Option<Duration>,
}

/// Política de admisión de verificaciones simultáneas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionPolicy {
    /// Verificaciones criptográficas simultáneas.
    pub max_concurrent: usize,
    /// Intentos que pueden esperar un slot antes de rechazar con saturación.
    pub queue_capacity: usize,
    /// Espera máxima antes de responder con saturación.
    pub max_wait: Duration,
}

/// Contador de intentos en espera; se libera al salir del ámbito.
struct Admission {
    capacity: usize,
    waiting: AtomicUsize,
}

/// Guardia de un intento encolado.
struct Waiting<'a>(&'a Admission);

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.waiting.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Admission {
    /// Reserva un lugar en la cola o falla de inmediato si está llena.
    fn join(&self) -> Result<Waiting<'_>, IdentityError> {
        let mut current = self.waiting.load(Ordering::Acquire);
        loop {
            if current >= self.capacity {
                return Err(IdentityError::VerificationSaturated);
            }
            match self.waiting.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(Waiting(self)),
                Err(observed) => current = observed,
            }
        }
    }
}

impl VerificationGate {
    /// Crea un límite explícito de verificaciones concurrentes.
    ///
    /// Sin cola acotada: los intentos esperan el slot tanto como haga falta. Es
    /// el comportamiento histórico del laboratorio y **no** debe usarse en
    /// producto; para eso existe [`Self::with_admission`].
    ///
    /// # Errors
    /// Devuelve [`IdentityError::StorageUnavailable`] si el límite es cero.
    pub fn new(max_concurrent: usize) -> Result<Self, IdentityError> {
        if max_concurrent == 0 {
            return Err(IdentityError::StorageUnavailable);
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(max_concurrent)),
            admission: None,
            max_wait: None,
        })
    }

    /// Crea un límite con cola acotada y espera máxima.
    ///
    /// # Errors
    /// Devuelve [`IdentityError::StorageUnavailable`] si algún valor es cero o
    /// la espera es nula.
    pub fn with_admission(policy: AdmissionPolicy) -> Result<Self, IdentityError> {
        if policy.max_concurrent == 0 || policy.queue_capacity == 0 || policy.max_wait.is_zero() {
            return Err(IdentityError::StorageUnavailable);
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(policy.max_concurrent)),
            admission: Some(Arc::new(Admission {
                capacity: policy.queue_capacity,
                waiting: AtomicUsize::new(0),
            })),
            max_wait: Some(policy.max_wait),
        })
    }

    /// Slots libres; útil para pruebas y telemetría acotada.
    pub fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }

    /// Intentos actualmente en cola.
    pub fn queued_permits(&self) -> usize {
        self.admission
            .as_ref()
            .map_or(0, |admission| admission.waiting.load(Ordering::Acquire))
    }

    /// Obtiene capacidad de verificación o falla con saturación.
    ///
    /// Un slot libre no cuenta como espera: la cola solo mide a quienes no
    /// pueden verificar de inmediato.
    async fn acquire(&self) -> Result<OwnedSemaphorePermit, IdentityError> {
        if let Ok(permit) = self.permits.clone().try_acquire_owned() {
            return Ok(permit);
        }
        // La reserva de cola se libera al terminar la espera, con éxito o error.
        let _waiting = match &self.admission {
            Some(admission) => Some(admission.join()?),
            None => None,
        };
        let pending = self.permits.clone().acquire_owned();
        let permit = match self.max_wait {
            Some(limit) => tokio::time::timeout(limit, pending)
                .await
                .map_err(|_| IdentityError::VerificationSaturated)?,
            None => pending.await,
        };
        permit.map_err(|_| IdentityError::StorageUnavailable)
    }

    async fn run<T, F>(&self, work: F) -> Result<T, IdentityError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = self.acquire().await?;
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
            "RESET_REQUIRED" | "PASSWORD_RESET_REQUIRED" => CredentialState::ResetRequired,
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

#[cfg(test)]
mod admission_tests {
    // Las fixtures verifican la política de admisión; los panics no son de producto.
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn policy(queue_capacity: usize, max_wait_millis: u64) -> AdmissionPolicy {
        AdmissionPolicy {
            max_concurrent: 1,
            queue_capacity,
            max_wait: Duration::from_millis(max_wait_millis),
        }
    }

    #[test]
    fn admission_queue_is_bounded_and_releases_its_slot() {
        let admission = Admission {
            capacity: 1,
            waiting: AtomicUsize::new(0),
        };
        let first = admission.join().expect("primer intento admitido");
        assert_eq!(admission.waiting.load(Ordering::Acquire), 1);
        assert!(matches!(
            admission.join(),
            Err(IdentityError::VerificationSaturated)
        ));
        drop(first);
        assert_eq!(admission.waiting.load(Ordering::Acquire), 0);
        assert!(admission.join().is_ok());
    }

    #[test]
    fn admission_policy_rejects_degenerate_limits() {
        assert!(VerificationGate::with_admission(policy(0, 50)).is_err());
        assert!(
            VerificationGate::with_admission(AdmissionPolicy {
                max_concurrent: 0,
                queue_capacity: 1,
                max_wait: Duration::from_millis(50),
            })
            .is_err()
        );
        assert!(
            VerificationGate::with_admission(AdmissionPolicy {
                max_concurrent: 1,
                queue_capacity: 1,
                max_wait: Duration::ZERO,
            })
            .is_err()
        );
    }

    #[tokio::test]
    async fn saturated_queue_and_expired_waiters_get_a_safe_error() {
        let gate = VerificationGate::with_admission(policy(1, 30)).expect("política válida");
        let held = gate.acquire().await.expect("slot libre");

        let waiter = tokio::spawn({
            let gate = gate.clone();
            async move { gate.acquire().await.map(|_| ()) }
        });
        for _ in 0..1_000 {
            if gate.queued_permits() == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(gate.queued_permits(), 1, "el segundo intento debe esperar");

        // Tercer intento con la cola llena: respuesta inmediata y segura.
        assert!(matches!(
            gate.acquire().await,
            Err(IdentityError::VerificationSaturated)
        ));
        assert!(matches!(
            waiter.await.expect("tarea de espera"),
            Err(IdentityError::VerificationSaturated)
        ));
        // Ni la cola ni los permisos quedan retenidos tras la saturación.
        assert_eq!(gate.queued_permits(), 0);

        drop(held);
        assert_eq!(gate.available_permits(), 1);
        assert!(gate.acquire().await.is_ok());
    }

    #[tokio::test]
    async fn a_free_slot_never_counts_as_queue() {
        let gate = VerificationGate::with_admission(policy(1, 1_000)).expect("política válida");
        let permit = gate.acquire().await.expect("slot libre");
        assert_eq!(gate.queued_permits(), 0);
        drop(permit);
    }
}
