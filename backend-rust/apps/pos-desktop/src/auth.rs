//! Autenticación IPC y custodia nativa de la sesión.
//!
//! El handle de sesión vive únicamente en memoria del proceso Rust. El renderer
//! recibe identidad y permisos, nunca el secreto ni su digest. Los comandos se
//! rechazan enteros en el modo laboratorio: no existe un camino sintético de
//! autenticación comercial.
//!
//! Ningún comando acepta actor, organización ni permisos desde JavaScript.

use std::time::{SystemTime, UNIX_EPOCH};

use pos_application::{
    identity::{LoginRequest, SessionIdentity},
    organization::list_authorized_branches,
};
use pos_core::{
    audit::AuditOccurredAt,
    identity::{NormalizedEmail, SessionHash, UtcSeconds},
    organization_branch::{ApplicationError, EntityId, IdentityGenerator},
};
use pos_credentials::SessionHandle;
use pos_storage_sqlite::{SqliteOrganizationBranchStore, session::identity_application};
use sqlx::SqlitePool;

use crate::{
    composition::{LocalState, ProductInstallation},
    dto::{
        BranchDto, IpcError, LoginRequestDto, LogoutAllDto, SelectBranchRequestDto,
        SessionIdentityDto,
    },
};

/// Vigencia de una sesión de turno: doce horas, dentro de la política admitida.
const SESSION_TTL_SECONDS: i64 = 12 * 60 * 60;

impl LocalState {
    /// Autentica al administrador y retiene el handle en memoria nativa.
    ///
    /// # Errors
    /// Devuelve [`IpcError::OperationNotAvailable`] en laboratorio,
    /// [`IpcError::Conflict`] si ya hay una sesión activa y
    /// [`IpcError::Unauthorized`] ante cualquier credencial rechazada.
    pub async fn auth_login(
        &self,
        request: LoginRequestDto,
    ) -> Result<SessionIdentityDto, IpcError> {
        let product = self.product_installation()?;
        let mut session = product.session.lock().await;
        if session.is_some() {
            return Err(IpcError::Conflict);
        }
        let email = NormalizedEmail::new(&request.email).map_err(|_| IpcError::Unauthorized)?;
        let (now, occurred_at) = clock(&product.pool).await?;
        let application = identity_application(product.pool.clone()).map_err(IpcError::from)?;
        let mut identities = SecureIdentities;
        let login = LoginRequest {
            email,
            now,
            occurred_at,
            session_ttl_seconds: SESSION_TTL_SECONDS,
        };
        let authenticated = application
            .login(&mut identities, &login, &request.password)
            .await
            .map_err(IpcError::from)?;
        *session = Some(authenticated.handle);
        Ok(SessionIdentityDto::from(authenticated.identity))
    }

    /// Devuelve la identidad y permisos de la sesión nativa vigente.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] si no hay sesión o dejó de ser válida.
    pub async fn auth_me(&self) -> Result<SessionIdentityDto, IpcError> {
        let product = self.product_installation()?;
        let Some(handle) = current_handle(product).await? else {
            return Err(IpcError::Unauthorized);
        };
        let hash = hash_of(&handle)?;
        let (now, _) = clock(&product.pool).await?;
        let application = identity_application(product.pool.clone()).map_err(IpcError::from)?;
        match application.current(&hash, now).await {
            Ok(identity) => Ok(SessionIdentityDto::from(identity)),
            Err(ApplicationError::Unauthenticated) => {
                forget_handle(product, &handle).await;
                Err(IpcError::Unauthorized)
            }
            Err(error) => Err(IpcError::from(error)),
        }
    }

    /// Revoca la sesión actual y libera el handle nativo.
    ///
    /// # Errors
    /// Devuelve [`IpcError::InternalError`] si SQLite no confirma la revocación.
    pub async fn auth_logout(&self) -> Result<(), IpcError> {
        let product = self.product_installation()?;
        let Some(handle) = current_handle(product).await? else {
            return Ok(());
        };
        let hash = hash_of(&handle)?;
        let (now, occurred_at) = clock(&product.pool).await?;
        let application = identity_application(product.pool.clone()).map_err(IpcError::from)?;
        let mut identities = SecureIdentities;
        let result = application
            .logout(&mut identities, &hash, now, &occurred_at)
            .await;
        forget_handle(product, &handle).await;
        result.map_err(IpcError::from)
    }

    /// Revoca todas las sesiones del usuario y libera el handle nativo.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] sin sesión vigente.
    pub async fn auth_logout_all(&self) -> Result<LogoutAllDto, IpcError> {
        let product = self.product_installation()?;
        let Some(handle) = current_handle(product).await? else {
            return Err(IpcError::Unauthorized);
        };
        let hash = hash_of(&handle)?;
        let (now, occurred_at) = clock(&product.pool).await?;
        let application = identity_application(product.pool.clone()).map_err(IpcError::from)?;
        let mut identities = SecureIdentities;
        let revoked = application
            .logout_all(&mut identities, &hash, now, &occurred_at)
            .await
            .map_err(IpcError::from)?;
        forget_handle(product, &handle).await;
        Ok(LogoutAllDto {
            revoked: u32::try_from(revoked).unwrap_or(u32::MAX),
        })
    }

    /// Selecciona una sucursal activa y devuelve los permisos de ese scope.
    ///
    /// # Errors
    /// Devuelve [`IpcError::NotFound`] si la sucursal no pertenece al tenant y
    /// [`IpcError::Forbidden`] si está inactiva o no es descubrible.
    pub async fn auth_select_branch(
        &self,
        request: SelectBranchRequestDto,
    ) -> Result<SessionIdentityDto, IpcError> {
        let product = self.product_installation()?;
        let Some(handle) = current_handle(product).await? else {
            return Err(IpcError::Unauthorized);
        };
        let hash = hash_of(&handle)?;
        let branch = EntityId::parse(&request.branch_id).map_err(|_| IpcError::ValidationError)?;
        let (now, occurred_at) = clock(&product.pool).await?;
        let application = identity_application(product.pool.clone()).map_err(IpcError::from)?;
        let mut identities = SecureIdentities;
        let identity = application
            .select_branch(&mut identities, &hash, now, &occurred_at, &branch)
            .await
            .map_err(IpcError::from)?;
        Ok(SessionIdentityDto::from(identity))
    }

    /// Lista las sucursales descubribles por la sesión actual.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] sin sesión vigente o sin `branch.read`.
    pub async fn auth_list_branches(&self) -> Result<Vec<BranchDto>, IpcError> {
        let product = self.product_installation()?;
        let Some(handle) = current_handle(product).await? else {
            return Err(IpcError::Unauthorized);
        };
        let hash = hash_of(&handle)?;
        let (now, _) = clock(&product.pool).await?;
        let application = identity_application(product.pool.clone()).map_err(IpcError::from)?;
        let principal = application
            .principal(&hash, now)
            .await
            .map_err(IpcError::from)?;
        let reader = SqliteOrganizationBranchStore::new(product.pool.clone());
        let branches = list_authorized_branches(&reader, &principal).await?;
        Ok(branches
            .into_iter()
            .map(|branch| BranchDto {
                id: branch.id().as_str().into(),
                code: branch.code().as_str().into(),
                name: branch.name().as_str().into(),
                status: branch.status().as_str().into(),
            })
            .collect())
    }
}

impl From<SessionIdentity> for SessionIdentityDto {
    fn from(identity: SessionIdentity) -> Self {
        Self {
            user_id: identity.user_id.as_str().into(),
            organization_id: identity.organization_id.as_str().into(),
            display_name: identity.display_name,
            email: identity.email,
            branch_id: identity.branch_id.map(|id| id.as_str().to_owned()),
            permissions: identity.permissions.into_iter().collect(),
        }
    }
}

/// Generador criptográfico de identidades para eventos de auditoría.
struct SecureIdentities;

impl IdentityGenerator for SecureIdentities {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        let value = pos_credentials::random_uuid_v4().map_err(|_| ApplicationError::Storage)?;
        EntityId::parse(&value).map_err(|_| ApplicationError::Storage)
    }
}

async fn current_handle(product: &ProductInstallation) -> Result<Option<String>, IpcError> {
    Ok(product.session.lock().await.clone())
}

async fn forget_handle(product: &ProductInstallation, handle: &str) {
    let mut session = product.session.lock().await;
    if session.as_deref() == Some(handle) {
        *session = None;
    }
}

fn hash_of(handle: &str) -> Result<SessionHash, IpcError> {
    SessionHandle::from_encoded(handle)
        .and_then(|value| value.hash())
        .map_err(|_| IpcError::Unauthorized)
}

pub(crate) async fn clock(pool: &SqlitePool) -> Result<(UtcSeconds, AuditOccurredAt), IpcError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IpcError::InternalError)?;
    let seconds = i64::try_from(elapsed.as_secs()).map_err(|_| IpcError::InternalError)?;
    let now = UtcSeconds::new(seconds).map_err(|_| IpcError::InternalError)?;
    let occurred_at: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')")
        .fetch_one(pool)
        .await
        .map_err(|_| IpcError::InternalError)?;
    let occurred_at = AuditOccurredAt::new(&occurred_at).map_err(|_| IpcError::InternalError)?;
    Ok((now, occurred_at))
}
