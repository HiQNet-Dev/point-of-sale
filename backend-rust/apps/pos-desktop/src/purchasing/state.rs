//! Composición del adaptador preparado. No acepta principal/actor desde IPC.
use crate::dto::IpcError;
use pos_application::purchasing::PurchasingApplication;
use pos_core::{
    audit::AuditOccurredAt,
    identity::{LocalPrincipalService, SessionHash, UtcSeconds, VerifiedPrincipal},
    organization_branch::{ApplicationError, EntityId, IdentityGenerator},
};
use pos_storage_sqlite::{SqliteUnitOfWork, identity::SqliteIdentityLab};
use sqlx::SqlitePool;

/// Contexto backend; no es serializable ni construible desde JavaScript.
///
/// Sin constructor productivo mientras A/D-AUTH-09/0016 estén pendientes. La
/// composición futura debe suministrar reloj real, generador seguro y la sesión
/// autenticada del backend, nunca IDs de actor enviados por el consumidor.
pub struct PurchasingState {
    pool: SqlitePool,
    session: SessionHash,
    clock: fn() -> Result<(UtcSeconds, AuditOccurredAt), IpcError>,
    next_id: fn() -> Result<EntityId, ApplicationError>,
}

pub(super) struct Context {
    pub application: PurchasingApplication<SqliteUnitOfWork>,
    pub principal: VerifiedPrincipal,
    pub occurred_at: AuditOccurredAt,
    pub identities: CommandIdentities,
}

pub(super) struct CommandIdentities(fn() -> Result<EntityId, ApplicationError>);
impl IdentityGenerator for CommandIdentities {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        (self.0)()
    }
}

impl PurchasingState {
    #[cfg(test)]
    pub(super) fn candidate(
        pool: SqlitePool,
        session: SessionHash,
        clock: fn() -> Result<(UtcSeconds, AuditOccurredAt), IpcError>,
        next_id: fn() -> Result<EntityId, ApplicationError>,
    ) -> Self {
        Self {
            pool,
            session,
            clock,
            next_id,
        }
    }

    /// Revalida sesión, revocación, expiración y grants SQLite en cada operación.
    pub(super) async fn resolve(&self, branch: Option<&str>) -> Result<Context, IpcError> {
        let branch = branch.map(super::dto::id).transpose()?;
        let (now, occurred_at) = (self.clock)()?;
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| IpcError::InternalError)?;
        let principal = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection))
            .authenticate(&self.session, now, branch.as_ref())
            .await
            .map_err(|_| IpcError::Unauthorized)?;
        drop(connection);
        Ok(Context {
            application: PurchasingApplication::new(SqliteUnitOfWork::new(self.pool.clone())),
            principal,
            occurred_at,
            identities: CommandIdentities(self.next_id),
        })
    }
}
