//! Adaptador de autenticación sobre el esquema productivo `0003`–`0006`.
//!
//! Implementa los puertos de [`pos_application::identity`] con SQLx y
//! `pos-credentials`. No define reglas de negocio: el bloqueo, la expiración, el
//! CAS y la atribución viven en `pos-core`/`pos-application`.
//!
//! # Custodia
//!
//! Solo se persiste el digest SHA-256 del handle. El handle crudo se devuelve al
//! shell nativo y nunca toca SQLite, archivos ni logs.

use pos_application::identity::{
    IdentityApplication, IdentityTransaction, IdentityUnitOfWork, IssuedSession, LoginDirectory,
    LoginSnapshot, PasswordGate, PasswordVerdict, SessionSecrets,
};
use pos_core::{
    identity::{
        CredentialKind, CredentialState, LocalCredential, LocalPrincipalService, LocalSession,
        NormalizedEmail, PosUser, SessionHash, UtcSeconds, VerifiedPrincipal,
    },
    organization_branch::{ApplicationError, Branch, EntityId},
};
use pos_credentials::{
    Argon2Policy, SessionHandle, VerificationOutcome, VerifyRequest, hash_password, random_uuid_v4,
    verify_credential,
};
use sqlx::SqlitePool;
use zeroize::Zeroizing;

use crate::{
    SqliteTransaction, SqliteUnitOfWork, credential::SqliteCredentialLab,
    identity::SqliteIdentityLab, organization_branch::BranchRow,
};

/// Aplicación de identidad lista para el composition root.
pub type SqliteIdentityApplication = IdentityApplication<
    SqliteUnitOfWork,
    Argon2PasswordGate,
    OsSessionSecrets,
    SqliteLoginDirectory,
>;

/// Ensambla los casos de uso con la política de bloqueo del producto.
///
/// # Errors
/// Devuelve [`ApplicationError::InvalidSessionInput`] si la política inyectada no
/// es válida.
pub fn identity_application(
    pool: SqlitePool,
) -> Result<SqliteIdentityApplication, ApplicationError> {
    let lockout = pos_application::identity::default_lockout_policy()?;
    Ok(IdentityApplication::new(
        SqliteUnitOfWork::new(pool.clone()),
        Argon2PasswordGate,
        OsSessionSecrets,
        SqliteLoginDirectory::new(pool),
        lockout,
    ))
}

/// Lecturas de identidad sobre el pool Local.
#[derive(Clone)]
pub struct SqliteLoginDirectory {
    pool: SqlitePool,
}

impl SqliteLoginDirectory {
    /// Enlaza el directorio al pool ya configurado.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl LoginDirectory for SqliteLoginDirectory {
    async fn find_login_snapshot(
        &self,
        email: &NormalizedEmail,
    ) -> Result<Option<LoginSnapshot>, ApplicationError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let candidates: Vec<(String, String)> =
            sqlx::query_as("SELECT id, organization_id FROM pos_user WHERE email = ?")
                .bind(email.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|_| ApplicationError::Storage)?;
        if candidates.len() != 1 {
            // Cero coincidencias o email ambiguo entre organizaciones: se trata
            // igual para no revelar existencia ni cardinalidad.
            return Ok(None);
        }
        let (user_id, organization_id) = &candidates[0];
        let user_id = EntityId::parse(user_id).map_err(|_| ApplicationError::Storage)?;
        let organization_id =
            EntityId::parse(organization_id).map_err(|_| ApplicationError::Storage)?;

        let user = {
            let mut lab = SqliteIdentityLab::canonical(&mut connection);
            lab.user_by_id(&organization_id, &user_id).await
        }
        .map_err(|_| ApplicationError::Storage)?;
        let credential = {
            let mut lab = SqliteCredentialLab::canonical(&mut connection);
            lab.load(&organization_id, &user_id, CredentialKind::Password)
                .await
        }
        .map_err(|_| ApplicationError::Storage)?;

        Ok(match (user, credential) {
            (Some(user), Some(credential)) => Some(LoginSnapshot { user, credential }),
            _ => None,
        })
    }

    async fn user_by_id(
        &self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Option<PosUser>, ApplicationError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let mut lab = SqliteIdentityLab::canonical(&mut connection);
        lab.user_by_id(organization_id, user_id)
            .await
            .map_err(|_| ApplicationError::Storage)
    }

    async fn principal_for_hash(
        &self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
    ) -> Result<VerifiedPrincipal, ApplicationError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let mut service = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection));
        service
            .authenticate(hash, now, branch_id)
            .await
            .map_err(|_| ApplicationError::Unauthenticated)
    }

    async fn branch_by_id(
        &self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Option<Branch>, ApplicationError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| ApplicationError::Storage)?;
        let row = sqlx::query_as::<_, BranchRow>(
            "SELECT id, organization_id, code, name, status, timezone_override, version \
             FROM branch WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(branch_id.as_str())
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| ApplicationError::Storage)?;
        row.map(BranchRow::into_domain)
            .transpose()
            .map_err(|_| ApplicationError::Storage)
    }
}

impl IdentityUnitOfWork for SqliteUnitOfWork {
    type Transaction = SqliteTransaction;

    async fn begin_transaction(&self) -> Result<SqliteTransaction, ApplicationError> {
        SqliteUnitOfWork::begin(self)
            .await
            .map_err(|_| ApplicationError::Storage)
    }

    async fn commit_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction
            .commit()
            .await
            .map_err(|_| ApplicationError::Storage)
    }

    async fn rollback_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction
            .rollback()
            .await
            .map_err(|_| ApplicationError::Storage)
    }
}

impl IdentityTransaction for SqliteTransaction {
    async fn reload_credential(
        &mut self,
        user: &PosUser,
    ) -> Result<Option<LocalCredential>, ApplicationError> {
        let connection = self.connection();
        let mut lab = SqliteCredentialLab::canonical(connection);
        lab.load(user.organization_id(), user.id(), CredentialKind::Password)
            .await
            .map_err(|_| ApplicationError::Storage)
    }

    async fn save_credential(
        &mut self,
        user: &PosUser,
        credential: &LocalCredential,
        expected_version: u64,
    ) -> Result<(), ApplicationError> {
        if credential.user_id() != user.id() {
            return Err(ApplicationError::OwnershipMismatch);
        }
        let state = match credential.state() {
            CredentialState::Active => "ACTIVE",
            CredentialState::ResetRequired => "PASSWORD_RESET_REQUIRED",
            // El esquema productivo no persiste DISABLED: la desactivación vive
            // en `pos_user.status`.
            CredentialState::Disabled => return Err(ApplicationError::Storage),
        };
        let expected = i64::try_from(expected_version).map_err(|_| ApplicationError::Storage)?;
        let version = i64::try_from(credential.credential_version())
            .map_err(|_| ApplicationError::Storage)?;
        let attempts = i64::from(credential.failed_login_attempts());
        let locked_until = credential.locked_until().map(UtcSeconds::get);
        let result = sqlx::query(
            "UPDATE local_credential SET state = ?, credential_version = ?, \
             failed_login_attempts = ?, \
             locked_until = CASE WHEN ? IS NULL THEN NULL \
                 ELSE strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch') END, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch') \
             WHERE organization_id = ? AND user_id = ? AND credential_kind = ? \
               AND credential_version = ?",
        )
        .bind(state)
        .bind(version)
        .bind(attempts)
        .bind(locked_until)
        .bind(locked_until)
        .bind(credential.updated_at().get())
        .bind(user.organization_id().as_str())
        .bind(user.id().as_str())
        .bind(kind_code(credential.kind()))
        .bind(expected)
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        if result.rows_affected() != 1 {
            return Err(ApplicationError::ConcurrentModification);
        }
        Ok(())
    }

    async fn insert_session(
        &mut self,
        session: &LocalSession,
        hash: &SessionHash,
    ) -> Result<(), ApplicationError> {
        let version =
            i64::try_from(session.session_version()).map_err(|_| ApplicationError::Storage)?;
        sqlx::query(
            "INSERT INTO local_session \
             (id, user_id, session_hash, state, created_at, expires_at, revoked_at, session_version) \
             VALUES (?, ?, ?, 'ACTIVE', strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch'), \
                 strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch'), NULL, ?)",
        )
        .bind(session.id().as_str())
        .bind(session.user_id().as_str())
        .bind(hash.as_storage_str())
        .bind(session.created_at().get())
        .bind(session.expires_at().get())
        .bind(version)
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(())
    }

    async fn principal_for_hash(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
    ) -> Result<VerifiedPrincipal, ApplicationError> {
        let connection = self.connection();
        let mut service = LocalPrincipalService::new(SqliteIdentityLab::canonical(connection));
        service
            .authenticate(hash, now, branch_id)
            .await
            .map_err(|_| ApplicationError::Unauthenticated)
    }

    async fn revoke_session(
        &mut self,
        principal: &VerifiedPrincipal,
        now: UtcSeconds,
    ) -> Result<u64, ApplicationError> {
        let result = sqlx::query(
            "UPDATE local_session SET state = 'REVOKED', \
             revoked_at = strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch'), \
             session_version = session_version + 1 \
             WHERE id = ? AND user_id = ? AND revoked_at IS NULL \
               AND EXISTS (SELECT 1 FROM pos_user u WHERE u.id = local_session.user_id \
                   AND u.organization_id = ?)",
        )
        .bind(now.get())
        .bind(principal.session_id().as_str())
        .bind(principal.user_id().as_str())
        .bind(principal.scope().organization_id().as_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(result.rows_affected())
    }

    async fn revoke_session_by_hash(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
    ) -> Result<u64, ApplicationError> {
        let result = sqlx::query(
            "UPDATE local_session SET state = 'REVOKED', \
             revoked_at = strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch'), \
             session_version = session_version + 1 \
             WHERE session_hash = ? AND revoked_at IS NULL",
        )
        .bind(now.get())
        .bind(hash.as_storage_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(result.rows_affected())
    }

    async fn revoke_all_sessions(
        &mut self,
        principal: &VerifiedPrincipal,
        now: UtcSeconds,
    ) -> Result<u64, ApplicationError> {
        let result = sqlx::query(
            "UPDATE local_session SET state = 'REVOKED', \
             revoked_at = strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'unixepoch'), \
             session_version = session_version + 1 \
             WHERE user_id = ? AND revoked_at IS NULL \
               AND EXISTS (SELECT 1 FROM pos_user u WHERE u.id = local_session.user_id \
                   AND u.organization_id = ?)",
        )
        .bind(now.get())
        .bind(principal.user_id().as_str())
        .bind(principal.scope().organization_id().as_str())
        .execute(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        Ok(result.rows_affected())
    }
}

/// Verificación Argon2id fuera del hilo de la transacción.
#[derive(Clone, Copy, Debug, Default)]
pub struct Argon2PasswordGate;

impl PasswordGate for Argon2PasswordGate {
    async fn verify(
        &self,
        credential: &LocalCredential,
        password: &str,
    ) -> Result<PasswordVerdict, ApplicationError> {
        let algorithm = credential.algorithm();
        let kind = credential.kind();
        let stored = credential.secret_hash().clone();
        let password = Zeroizing::new(password.to_owned());
        let outcome = tokio::task::spawn_blocking(move || {
            let request = VerifyRequest {
                purpose: kind,
                algorithm,
                stored: &stored,
                password: password.as_str(),
            };
            verify_credential(&request)
        })
        .await
        .map_err(|_| ApplicationError::Storage)?
        .map_err(|_| ApplicationError::Storage)?;
        Ok(match outcome {
            VerificationOutcome::Verified { .. } => PasswordVerdict::Verified,
            // Un material ilegible se trata como secreto incorrecto: no se
            // ofrece un camino de recuperación implícito.
            VerificationOutcome::Rejected | VerificationOutcome::Unusable => {
                PasswordVerdict::Rejected
            }
        })
    }

    async fn absorb_absent(&self, password: &str) -> Result<(), ApplicationError> {
        let password = Zeroizing::new(password.to_owned());
        tokio::task::spawn_blocking(move || {
            // Coste comparable al de una verificación real; el resultado se
            // descarta a propósito.
            let _ = hash_password(password.as_str(), Argon2Policy::LAB_V1);
        })
        .await
        .map_err(|_| ApplicationError::Storage)
    }
}

/// Emisión de handles con CSPRNG del sistema operativo.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsSessionSecrets;

impl SessionSecrets for OsSessionSecrets {
    fn issue(
        &self,
        user_id: EntityId,
        organization_id: EntityId,
        now: UtcSeconds,
        ttl_seconds: i64,
    ) -> Result<IssuedSession, ApplicationError> {
        let handle = SessionHandle::generate().map_err(|_| ApplicationError::Storage)?;
        let hash = handle.hash().map_err(|_| ApplicationError::Storage)?;
        let session_id = EntityId::parse(&random_uuid_v4().map_err(|_| ApplicationError::Storage)?)
            .map_err(|_| ApplicationError::Storage)?;
        let expires_at = now
            .get()
            .checked_add(ttl_seconds)
            .ok_or(ApplicationError::InvalidSessionInput)
            .and_then(|value| {
                UtcSeconds::new(value).map_err(|_| ApplicationError::InvalidSessionInput)
            })?;
        let session = LocalSession::create(
            session_id,
            user_id,
            organization_id,
            hash.clone(),
            now,
            expires_at,
        )
        .map_err(|_| ApplicationError::InvalidSessionInput)?;
        Ok(IssuedSession {
            session,
            hash,
            handle: handle.as_client_secret().to_owned(),
        })
    }
}

fn kind_code(kind: CredentialKind) -> &'static str {
    match kind {
        CredentialKind::Password => "PASSWORD",
        CredentialKind::Pin => "PIN",
    }
}
