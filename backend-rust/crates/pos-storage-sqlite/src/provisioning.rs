//! Adaptador candidato de autorizaciones de provisioning.
//!
//! Trabaja sobre el esquema propuesto `0017`, que **no** forma parte del
//! migrador productivo. Reutiliza [`SqliteUnitOfWork`] (`BEGIN IMMEDIATE`) para
//! que emisión, revocación y consumo sean mutaciones auditables de una sola
//! conexión. El token en claro nunca entra aquí: solo su digest.

use std::fmt;

use pos_core::{
    identity::{
        ProvisioningAuthorization, ProvisioningError, ProvisioningPurpose, ProvisioningState,
    },
    organization_branch::EntityId,
};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};

use crate::{SqliteTransaction, SqliteUnitOfWork};

/// Fallo del adaptador de provisioning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningStorageError {
    /// Fallo de persistencia no clasificable.
    Database,
    /// El dominio rechazó la operación.
    Rejected(ProvisioningError),
    /// Ya existe un derecho vivo para la misma instalación y finalidad.
    AlreadyLive,
    /// La autorización no existe o ya no admite la transición.
    NotLive,
}

impl fmt::Display for ProvisioningStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Database => "persistencia de provisioning fallida",
            Self::Rejected(_) => "autorización de provisioning rechazada",
            Self::AlreadyLive => "ya existe un derecho vivo para la instalación",
            Self::NotLive => "la autorización no está viva",
        })
    }
}

impl std::error::Error for ProvisioningStorageError {}

impl From<ProvisioningError> for ProvisioningStorageError {
    fn from(value: ProvisioningError) -> Self {
        Self::Rejected(value)
    }
}

#[derive(FromRow)]
struct AuthorizationRow {
    id: String,
    installation_id: String,
    purpose: String,
    token_digest: String,
    issued_at: i64,
    expires_at: i64,
    state: String,
    closed_at: Option<i64>,
}

impl AuthorizationRow {
    fn into_domain(self) -> Result<ProvisioningAuthorization, ProvisioningStorageError> {
        Ok(ProvisioningAuthorization::rehydrate(
            EntityId::parse(&self.id).map_err(|_| ProvisioningStorageError::Database)?,
            EntityId::parse(&self.installation_id)
                .map_err(|_| ProvisioningStorageError::Database)?,
            ProvisioningPurpose::parse(&self.purpose)?,
            &self.token_digest,
            self.issued_at,
            self.expires_at,
            ProvisioningState::parse(&self.state)?,
            self.closed_at,
        )?)
    }
}

const SELECT_BY_ID: &str = "SELECT id, installation_id, purpose, token_digest, issued_at, \
     expires_at, state, closed_at FROM bootstrap_authorization WHERE id = ?";
const SELECT_STALE: &str = "SELECT id, installation_id, purpose, token_digest, issued_at, \
     expires_at, state, closed_at FROM bootstrap_authorization \
     WHERE installation_id = ? AND purpose = ? AND state = 'PENDING' AND expires_at <= ?";

/// Repositorio candidato de autorizaciones, sin activación comercial.
#[derive(Clone)]
pub struct SqliteProvisioningStore {
    unit_of_work: SqliteUnitOfWork,
    pool: SqlitePool,
}

impl SqliteProvisioningStore {
    /// Crea el repositorio sobre el pool Local ya configurado.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            unit_of_work: SqliteUnitOfWork::new(pool.clone()),
            pool,
        }
    }

    /// Emite una autorización pendiente y registra `ISSUED`.
    ///
    /// Primero materializa como `EXPIRED` cualquier derecho vivo cuya ventana ya
    /// terminó, de modo que se pueda reemitir tras la caducidad. Un derecho vivo
    /// todavía válido bloquea la emisión con [`ProvisioningStorageError::AlreadyLive`].
    pub async fn issue(
        &self,
        authorization: &ProvisioningAuthorization,
        event_id: &EntityId,
        now: i64,
    ) -> Result<(), ProvisioningStorageError> {
        let mut transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| ProvisioningStorageError::Database)?;
        expire_stale(
            &mut transaction,
            authorization.installation_id(),
            authorization.purpose(),
            now,
        )
        .await?;
        sqlx::query(
            "INSERT INTO bootstrap_authorization \
             (id, installation_id, purpose, token_digest, issued_at, expires_at, state, \
              closed_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, 'PENDING', NULL, ?)",
        )
        .bind(authorization.id().as_str())
        .bind(authorization.installation_id().as_str())
        .bind(authorization.purpose().as_str())
        .bind(authorization.token_digest())
        .bind(authorization.issued_at())
        .bind(authorization.expires_at())
        .bind(now)
        .execute(transaction.connection())
        .await
        .map_err(map_insert_error)?;
        append_event(
            &mut transaction,
            event_id,
            authorization.id(),
            "ISSUED",
            now,
            None,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|_| ProvisioningStorageError::Database)
    }

    /// Lee una autorización; las lecturas no abren UoW de escritura.
    pub async fn get(
        &self,
        id: &EntityId,
    ) -> Result<Option<ProvisioningAuthorization>, ProvisioningStorageError> {
        let row = sqlx::query_as::<_, AuthorizationRow>(SELECT_BY_ID)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| ProvisioningStorageError::Database)?;
        row.map(AuthorizationRow::into_domain).transpose()
    }

    /// Revoca una autorización pendiente y registra `REVOKED` si cambió.
    ///
    /// Idempotente: revocar dos veces no duplica eventos ni falla.
    pub async fn revoke(
        &self,
        id: &EntityId,
        event_id: &EntityId,
        now: i64,
    ) -> Result<ProvisioningState, ProvisioningStorageError> {
        let mut transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| ProvisioningStorageError::Database)?;
        let mut authorization = load(&mut transaction, id).await?;
        let before = authorization.state();
        authorization.revoke(now)?;
        if authorization.state() == before {
            transaction
                .commit()
                .await
                .map_err(|_| ProvisioningStorageError::Database)?;
            return Ok(before);
        }
        let updated = sqlx::query(
            "UPDATE bootstrap_authorization SET state = 'REVOKED', closed_at = ?, updated_at = ? \
             WHERE id = ? AND state = 'PENDING'",
        )
        .bind(now)
        .bind(now)
        .bind(id.as_str())
        .execute(transaction.connection())
        .await
        .map_err(|_| ProvisioningStorageError::Database)?;
        if updated.rows_affected() != 1 {
            return Err(ProvisioningStorageError::NotLive);
        }
        append_event(
            &mut transaction,
            event_id,
            id,
            "REVOKED",
            now,
            Some("revoked=native"),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|_| ProvisioningStorageError::Database)?;
        Ok(authorization.state())
    }

    /// Consume una autorización dentro de la transacción del llamador.
    ///
    /// El consumo debe confirmarse en la misma UoW que el bootstrap. El CAS
    /// (`state = 'PENDING'`) garantiza que solo una transición afecta una fila.
    pub async fn consume_in(
        transaction: &mut SqliteTransaction,
        id: &EntityId,
        token_digest: &str,
        installation_id: &EntityId,
        purpose: ProvisioningPurpose,
        event_id: &EntityId,
        now: i64,
    ) -> Result<ProvisioningAuthorization, ProvisioningStorageError> {
        let mut authorization = load(transaction, id).await?;
        authorization.consume(token_digest, installation_id, purpose, now)?;
        let updated = sqlx::query(
            "UPDATE bootstrap_authorization SET state = 'CONSUMED', closed_at = ?, updated_at = ? \
             WHERE id = ? AND state = 'PENDING'",
        )
        .bind(now)
        .bind(now)
        .bind(id.as_str())
        .execute(transaction.connection())
        .await
        .map_err(|_| ProvisioningStorageError::Database)?;
        if updated.rows_affected() != 1 {
            return Err(ProvisioningStorageError::NotLive);
        }
        append_event(
            transaction,
            event_id,
            id,
            "CONSUMED",
            now,
            Some("bootstrap=confirmed"),
        )
        .await?;
        Ok(authorization)
    }
}

pub(crate) async fn consume_derived(
    transaction: &mut SqliteTransaction,
    id: &EntityId,
    token_digest: &str,
    installation_id: &EntityId,
    purpose: ProvisioningPurpose,
    now: i64,
) -> Result<ProvisioningAuthorization, ProvisioningStorageError> {
    let event_id = derived_event_id(id.as_str(), "CONSUMED")?;
    SqliteProvisioningStore::consume_in(
        transaction,
        id,
        token_digest,
        installation_id,
        purpose,
        &event_id,
        now,
    )
    .await
}

async fn load(
    transaction: &mut SqliteTransaction,
    id: &EntityId,
) -> Result<ProvisioningAuthorization, ProvisioningStorageError> {
    let row = sqlx::query_as::<_, AuthorizationRow>(SELECT_BY_ID)
        .bind(id.as_str())
        .fetch_optional(transaction.connection())
        .await
        .map_err(|_| ProvisioningStorageError::Database)?
        .ok_or(ProvisioningStorageError::NotLive)?;
    row.into_domain()
}

async fn expire_stale(
    transaction: &mut SqliteTransaction,
    installation_id: &EntityId,
    purpose: ProvisioningPurpose,
    now: i64,
) -> Result<(), ProvisioningStorageError> {
    let stale = sqlx::query_as::<_, AuthorizationRow>(SELECT_STALE)
        .bind(installation_id.as_str())
        .bind(purpose.as_str())
        .bind(now)
        .fetch_all(transaction.connection())
        .await
        .map_err(|_| ProvisioningStorageError::Database)?;
    for row in stale {
        let authorization_id =
            EntityId::parse(&row.id).map_err(|_| ProvisioningStorageError::Database)?;
        let event_id = derived_event_id(authorization_id.as_str(), "EXPIRED")?;
        sqlx::query(
            "UPDATE bootstrap_authorization SET state = 'EXPIRED', closed_at = expires_at, \
             updated_at = ? WHERE id = ? AND state = 'PENDING'",
        )
        .bind(now)
        .bind(&row.id)
        .execute(transaction.connection())
        .await
        .map_err(|_| ProvisioningStorageError::Database)?;
        append_event(
            transaction,
            &event_id,
            &authorization_id,
            "EXPIRED",
            now,
            Some("expiry=materialized"),
        )
        .await?;
    }
    Ok(())
}

async fn append_event(
    transaction: &mut SqliteTransaction,
    event_id: &EntityId,
    authorization_id: &EntityId,
    event_type: &str,
    occurred_at: i64,
    context: Option<&str>,
) -> Result<(), ProvisioningStorageError> {
    sqlx::query(
        "INSERT INTO bootstrap_authorization_event \
         (id, authorization_id, event_type, occurred_at, security_context) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(event_id.as_str())
    .bind(authorization_id.as_str())
    .bind(event_type)
    .bind(occurred_at)
    .bind(context)
    .execute(transaction.connection())
    .await
    .map(|_| ())
    .map_err(|_| ProvisioningStorageError::Database)
}

fn map_insert_error(error: sqlx::Error) -> ProvisioningStorageError {
    if error
        .as_database_error()
        .is_some_and(|database| database.is_unique_violation())
    {
        ProvisioningStorageError::AlreadyLive
    } else {
        ProvisioningStorageError::Database
    }
}

/// Identificador determinista de evento para transiciones internas.
///
/// La caducidad se materializa sin un generador inyectado; derivar el id del
/// propio derecho y del tipo de evento evita colisiones entre derechos y es
/// estable ante reintentos.
fn derived_event_id(
    authorization_id: &str,
    event_type: &str,
) -> Result<EntityId, ProvisioningStorageError> {
    let mut hasher = Sha256::new();
    hasher.update(authorization_id.as_bytes());
    hasher.update(b":");
    hasher.update(event_type.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    EntityId::parse(&format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
    .map_err(|_| ProvisioningStorageError::Database)
}
