//! Casos de uso de autenticación Local: login, identidad, sucursal y logout.
//!
//! # Invariantes
//!
//! - El usuario se resuelve por email normalizado; la organización procede de la
//!   fila del usuario, nunca de la petición.
//! - Todos los rechazos de autenticación devuelven
//!   [`ApplicationError::InvalidCredentials`]: no se distinguen usuario
//!   inexistente, cuenta inactiva, credencial bloqueada o contraseña errónea.
//! - El handle de sesión se entrega una sola vez y su digest es lo único que se
//!   persiste.
//! - Los permisos se resuelven en SQLite dentro de la misma unidad de trabajo que
//!   crea o revalida la sesión.

use pos_core::{
    audit::{AuditEventId, AuditEventRecord, AuditOccurredAt, AuditReason},
    identity::{
        CredentialState, LockoutPolicy, PosUser, SessionHash, UtcSeconds, VerifiedPrincipal,
    },
    organization_branch::{ApplicationError, EntityId, IdentityGenerator, Status},
};
use serde_json::json;

use crate::{
    audit::{AuditAppender, append_audit},
    identity::{dto::*, ports::*},
};

/// Vigencia mínima admitida para una sesión, en segundos.
pub const MIN_SESSION_TTL_SECONDS: i64 = 60;
/// Vigencia máxima admitida para una sesión, en segundos.
pub const MAX_SESSION_TTL_SECONDS: i64 = 86_400;

/// Política de bloqueo equivalente a la del backend Java (5 intentos, 15 min).
pub fn default_lockout_policy() -> Result<LockoutPolicy, ApplicationError> {
    LockoutPolicy::new(5, 900).map_err(|_| ApplicationError::InvalidSessionInput)
}

/// Casos de uso de identidad sobre una unidad de trabajo y puertos inyectados.
pub struct IdentityApplication<U, G, S, D> {
    unit_of_work: U,
    gate: G,
    secrets: S,
    directory: D,
    lockout: LockoutPolicy,
}

impl<U, G, S, D> IdentityApplication<U, G, S, D>
where
    U: IdentityUnitOfWork,
    G: PasswordGate,
    S: SessionSecrets,
    D: LoginDirectory,
{
    /// Ensambla los casos de uso con la política de bloqueo aprobada por el
    /// composition root.
    pub fn new(unit_of_work: U, gate: G, secrets: S, directory: D, lockout: LockoutPolicy) -> Self {
        Self {
            unit_of_work,
            gate,
            secrets,
            directory,
            lockout,
        }
    }

    /// Autentica con contraseña y emite una sesión nueva.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::InvalidCredentials`] ante cualquier causa de
    /// rechazo, [`ApplicationError::InvalidSessionInput`] si la vigencia está
    /// fuera de política y [`ApplicationError::Storage`] o
    /// [`ApplicationError::Audit`] si la persistencia falla.
    pub async fn login<I: IdentityGenerator>(
        &self,
        identities: &mut I,
        request: &LoginRequest,
        password: &str,
    ) -> Result<AuthenticatedSession, ApplicationError> {
        if !(MIN_SESSION_TTL_SECONDS..=MAX_SESSION_TTL_SECONDS)
            .contains(&request.session_ttl_seconds)
        {
            return Err(ApplicationError::InvalidSessionInput);
        }

        let Some(snapshot) = self.directory.find_login_snapshot(&request.email).await? else {
            // Mismo costo criptográfico que una verificación real.
            self.gate.absorb_absent(password).await?;
            return Err(ApplicationError::InvalidCredentials);
        };
        if !snapshot.user.is_active() {
            self.gate.absorb_absent(password).await?;
            return Err(ApplicationError::InvalidCredentials);
        }
        if snapshot
            .credential
            .accepts_verification(request.now)
            .is_err()
        {
            self.gate.absorb_absent(password).await?;
            return Err(ApplicationError::InvalidCredentials);
        }

        match self.gate.verify(&snapshot.credential, password).await? {
            PasswordVerdict::Rejected => {
                // El fallo se registra y audita en su propia UoW; si esa escritura
                // falla, el login sigue rechazado y no se concede nada.
                let _ = self
                    .record_failure(identities, &snapshot.user, request)
                    .await;
                return Err(ApplicationError::InvalidCredentials);
            }
            PasswordVerdict::Verified => {}
        }

        let issued = self.secrets.issue(
            snapshot.user.id().clone(),
            snapshot.user.organization_id().clone(),
            request.now,
            request.session_ttl_seconds,
        )?;
        let expected = snapshot.credential.credential_version();

        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let mut current = transaction
                .reload_credential(&snapshot.user)
                .await?
                .ok_or(ApplicationError::InvalidCredentials)?;
            if current.credential_version() != expected {
                // El material cambió entre la verificación y la escritura.
                return Err(ApplicationError::InvalidCredentials);
            }
            current
                .accepts_verification(request.now)
                .map_err(|_| ApplicationError::InvalidCredentials)?;
            if current
                .record_successful_login(request.now, expected)
                .map_err(|_| ApplicationError::Storage)?
            {
                transaction
                    .save_credential(&snapshot.user, &current, expected)
                    .await?;
            }
            transaction
                .insert_session(&issued.session, &issued.hash)
                .await?;
            let principal = transaction
                .principal_for_hash(&issued.hash, request.now, None)
                .await?;
            append_audit(
                &mut transaction,
                identities,
                &principal,
                "auth.login",
                "local_session",
                principal.session_id().clone(),
                json!({ "method": "password" }),
                &request.occurred_at,
            )
            .await?;
            Ok(identity_of(&principal, &snapshot.user))
        }
        .await;
        let identity = finish(&self.unit_of_work, transaction, operation).await?;
        Ok(AuthenticatedSession {
            identity,
            handle: issued.handle,
        })
    }

    /// Revalida una sesión y devuelve el principal autoritativo.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Unauthenticated`] si la sesión no existe,
    /// venció o fue revocada.
    pub async fn principal(
        &self,
        hash: &SessionHash,
        now: UtcSeconds,
    ) -> Result<VerifiedPrincipal, ApplicationError> {
        self.directory
            .principal_for_hash(hash, now, None)
            .await
            .map_err(unauthenticated)
    }

    /// Revalida una sesión y devuelve su identidad y permisos actuales.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Unauthenticated`] si la sesión no existe,
    /// venció o fue revocada.
    pub async fn current(
        &self,
        hash: &SessionHash,
        now: UtcSeconds,
    ) -> Result<SessionIdentity, ApplicationError> {
        let principal = self
            .directory
            .principal_for_hash(hash, now, None)
            .await
            .map_err(unauthenticated)?;
        let user = self
            .directory
            .user_by_id(principal.scope().organization_id(), principal.user_id())
            .await?
            .ok_or(ApplicationError::Unauthenticated)?;
        Ok(identity_of(&principal, &user))
    }

    /// Selecciona una sucursal activa de la organización del principal.
    ///
    /// La sucursal debe existir en la organización, estar activa y ser
    /// descubrible con `branch.read` en el scope resultante. Una sucursal de otra
    /// organización no se distingue de una inexistente.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Unauthenticated`] sin sesión vigente,
    /// [`ApplicationError::BranchNotFound`] si no pertenece al tenant,
    /// [`ApplicationError::Forbidden`] si está inactiva o no es descubrible.
    pub async fn select_branch<I: IdentityGenerator>(
        &self,
        identities: &mut I,
        hash: &SessionHash,
        now: UtcSeconds,
        occurred_at: &AuditOccurredAt,
        branch_id: &EntityId,
    ) -> Result<SessionIdentity, ApplicationError> {
        let organization_principal = self
            .directory
            .principal_for_hash(hash, now, None)
            .await
            .map_err(unauthenticated)?;
        let organization_id = organization_principal.scope().organization_id().clone();
        let branch = self
            .directory
            .branch_by_id(&organization_id, branch_id)
            .await?
            .ok_or(ApplicationError::BranchNotFound)?;
        if branch.status() != Status::Active {
            return Err(ApplicationError::Forbidden);
        }
        let principal = self
            .directory
            .principal_for_hash(hash, now, Some(branch_id))
            .await
            .map_err(unauthenticated)?;
        let code = pos_core::identity::PermissionCode::new("branch.read")
            .map_err(|_| ApplicationError::Forbidden)?;
        principal
            .authorize(&code, principal.scope())
            .map_err(|_| ApplicationError::Forbidden)?;
        let user = self
            .directory
            .user_by_id(&organization_id, principal.user_id())
            .await?
            .ok_or(ApplicationError::Unauthenticated)?;

        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = append_audit(
            &mut transaction,
            identities,
            &principal,
            "auth.branch_selected",
            "branch",
            branch_id.clone(),
            json!({ "branch": branch_id.as_str() }),
            occurred_at,
        )
        .await;
        finish(&self.unit_of_work, transaction, operation).await?;
        Ok(identity_of(&principal, &user))
    }

    /// Revoca la sesión actual.
    ///
    /// Es idempotente: una sesión ya revocada o vencida no produce error, pero
    /// tampoco se audita de nuevo.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Storage`] si la revocación no se confirma.
    pub async fn logout<I: IdentityGenerator>(
        &self,
        identities: &mut I,
        hash: &SessionHash,
        now: UtcSeconds,
        occurred_at: &AuditOccurredAt,
    ) -> Result<(), ApplicationError> {
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let principal = transaction.principal_for_hash(hash, now, None).await;
            let Ok(principal) = principal else {
                // Sesión vencida o ya revocada: se limpia sin auditar.
                transaction.revoke_session_by_hash(hash, now).await?;
                return Ok(());
            };
            let revoked = transaction.revoke_session(&principal, now).await?;
            if revoked > 0 {
                append_audit(
                    &mut transaction,
                    identities,
                    &principal,
                    "auth.logout",
                    "local_session",
                    principal.session_id().clone(),
                    json!({}),
                    occurred_at,
                )
                .await?;
            }
            Ok(())
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Revoca todas las sesiones vigentes del usuario autenticado.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Unauthenticated`] sin sesión vigente y
    /// [`ApplicationError::Storage`] si la revocación no se confirma.
    pub async fn logout_all<I: IdentityGenerator>(
        &self,
        identities: &mut I,
        hash: &SessionHash,
        now: UtcSeconds,
        occurred_at: &AuditOccurredAt,
    ) -> Result<u64, ApplicationError> {
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let principal = transaction
                .principal_for_hash(hash, now, None)
                .await
                .map_err(unauthenticated)?;
            let revoked = transaction.revoke_all_sessions(&principal, now).await?;
            append_audit(
                &mut transaction,
                identities,
                &principal,
                "auth.logout_all",
                "pos_user",
                principal.user_id().clone(),
                json!({ "revoked": revoked }),
                occurred_at,
            )
            .await?;
            Ok(revoked)
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }

    /// Incrementa el contador de intentos fallidos con CAS y audita el rechazo.
    async fn record_failure<I: IdentityGenerator>(
        &self,
        identities: &mut I,
        user: &PosUser,
        request: &LoginRequest,
    ) -> Result<(), ApplicationError> {
        let event_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let operation = async {
            let Some(mut credential) = transaction.reload_credential(user).await? else {
                return Ok(());
            };
            if credential.state() != CredentialState::Active
                || credential.accepts_verification(request.now).is_err()
            {
                return Ok(());
            }
            let expected = credential.credential_version();
            credential
                .record_failed_login(request.now, self.lockout, expected)
                .map_err(|_| ApplicationError::Storage)?;
            transaction
                .save_credential(user, &credential, expected)
                .await?;
            let reason =
                AuditReason::new("invalid_credentials").map_err(|_| ApplicationError::Audit)?;
            let event = AuditEventRecord::authentication_failed(
                AuditEventId::new(event_id),
                user.organization_id().clone(),
                user.id().clone(),
                reason,
                request.occurred_at.clone(),
            )
            .map_err(|_| ApplicationError::Audit)?;
            transaction.append_audit_event(&event).await
        }
        .await;
        finish(&self.unit_of_work, transaction, operation).await
    }
}

fn identity_of(principal: &VerifiedPrincipal, user: &PosUser) -> SessionIdentity {
    SessionIdentity {
        user_id: principal.user_id().clone(),
        organization_id: principal.scope().organization_id().clone(),
        display_name: user.display_name().as_str().to_owned(),
        email: user.email().as_str().to_owned(),
        branch_id: principal.scope().branch_id().cloned(),
        permissions: principal
            .permissions()
            .map(|code| code.as_str().to_owned())
            .collect(),
    }
}

fn unauthenticated(error: ApplicationError) -> ApplicationError {
    match error {
        ApplicationError::Forbidden | ApplicationError::Unauthenticated => {
            ApplicationError::Unauthenticated
        }
        other => other,
    }
}

async fn finish<U, T>(
    unit_of_work: &U,
    transaction: U::Transaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError>
where
    U: IdentityUnitOfWork,
{
    match operation {
        Ok(value) => {
            unit_of_work.commit_transaction(transaction).await?;
            Ok(value)
        }
        Err(error) => {
            if unit_of_work
                .rollback_transaction(transaction)
                .await
                .is_err()
            {
                return Err(ApplicationError::Storage);
            }
            Err(error)
        }
    }
}
