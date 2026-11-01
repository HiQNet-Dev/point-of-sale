//! Primera instalación Free Local: detección, autorización nativa y bootstrap.
//!
//! El token de provisioning se lee del archivo privado entregado por el emisor
//! administrativo. Nunca viaja por IPC, JavaScript, argumentos, logs ni URLs.
//!
//! # Estados
//!
//! [`LocalState::bootstrap_status`] clasifica la instalación sin consumir nada.
//! [`LocalState::bootstrap_install`] solo actúa sobre un derecho vivo y deja la
//! autorización consumida en la misma transacción que crea el administrador.
//!
//! La custodia del archivo la resuelve cada plataforma: bits POSIX en Unix y
//! propietario/DACL verificados sobre el handle en Windows (ADR-0029); el resto
//! de plataformas responde `UNSUPPORTED_PLATFORM`.

use pos_application::bootstrap::{
    BootstrapRequest, ProvisioningProof, provision_initial_installation_with_proof,
    requires_provisioning,
};
use pos_core::{
    audit::{AuditEventId, BootstrapError, BootstrapSecretVerifier, authorize_bootstrap},
    identity::{NormalizedEmail, UtcSeconds},
    organization_branch::{
        BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewOrganization, Timezone,
    },
};
use pos_credentials::{Argon2Policy, ProvisioningToken, random_uuid_v4};
use pos_storage_sqlite::{
    SqliteUnitOfWork,
    bootstrap::CandidateBootstrapPasswords,
    native_authorization::{
        AuthorizationFileError, NATIVE_AUTHORIZATION_FILE, NativeAuthorization,
        NativeAuthorizationState, native_authorization_state, read_native_authorization,
    },
};

use crate::{
    auth::clock,
    composition::{LocalState, ProductInstallation},
    dto::{BootstrapInstallRequestDto, BootstrapStatusDto, IpcError},
};

/// Longitud mínima de la contraseña inicial de Free Local.
///
/// Es una política de producto explícita, no un valor criptográfico: Argon2id
/// solo impone el máximo. Se documenta en ADR-0028.
pub(crate) const MIN_INITIAL_PASSWORD_LEN: usize = 12;

impl LocalState {
    /// Clasifica el estado de la primera instalación sin consumir la autorización.
    ///
    /// # Errors
    /// Devuelve [`IpcError::OperationNotAvailable`] en laboratorio y
    /// [`IpcError::InternalError`] si SQLite no responde.
    pub async fn bootstrap_status(&self) -> Result<BootstrapStatusDto, IpcError> {
        let product = self.product_installation()?;
        if !needs_bootstrap(product).await? {
            return Ok(status("ALREADY_PROVISIONED", None));
        }
        let (now, _) = clock(&product.pool).await?;
        match read_native_authorization(&product.directory) {
            Err(AuthorizationFileError::UnsupportedPlatform) => {
                Ok(status("UNSUPPORTED_PLATFORM", None))
            }
            Err(AuthorizationFileError::Missing) => Ok(status("AWAITING_AUTHORIZATION", None)),
            Err(AuthorizationFileError::Insecure) => Ok(status("INSECURE_PERMISSIONS", None)),
            Err(AuthorizationFileError::Invalid) => Ok(status("INVALID_AUTHORIZATION", None)),
            Ok(authorization) => {
                let state = native_authorization_state(&product.pool, &authorization, now.get())
                    .await
                    .map_err(|_| IpcError::InternalError)?;
                Ok(match state {
                    NativeAuthorizationState::Ready { expires_at } => {
                        status("READY", u32::try_from(expires_at).ok())
                    }
                    NativeAuthorizationState::Expired => status("AUTHORIZATION_EXPIRED", None),
                    NativeAuthorizationState::Revoked => status("AUTHORIZATION_REVOKED", None),
                    NativeAuthorizationState::Consumed => status("AUTHORIZATION_CONSUMED", None),
                    NativeAuthorizationState::Unknown => status("INVALID_AUTHORIZATION", None),
                })
            }
        }
    }

    /// Completa la primera instalación consumiendo la autorización nativa.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Conflict`] si la instalación ya tiene identidad,
    /// [`IpcError::Unauthorized`] si la autorización falta o no está viva,
    /// [`IpcError::Forbidden`] si el archivo no es privado,
    /// [`IpcError::ValidationError`] si los datos no cumplen el contrato,
    /// [`IpcError::OperationNotAvailable`] en laboratorio o en plataformas sin
    /// custodia verificable, y [`IpcError::InternalError`] si la persistencia
    /// falla.
    pub async fn bootstrap_install(
        &self,
        request: BootstrapInstallRequestDto,
    ) -> Result<(), IpcError> {
        let product = self.product_installation()?;
        if !needs_bootstrap(product).await? {
            return Err(IpcError::Conflict);
        }
        if request.password.chars().count() < MIN_INITIAL_PASSWORD_LEN {
            return Err(IpcError::ValidationError);
        }
        let authorization = read_native_authorization(&product.directory).map_err(file_error)?;
        let (now, occurred_at) = clock(&product.pool).await?;
        if !matches!(
            native_authorization_state(&product.pool, &authorization, now.get())
                .await
                .map_err(|_| IpcError::InternalError)?,
            NativeAuthorizationState::Ready { .. }
        ) {
            return Err(IpcError::Unauthorized);
        }

        let capability = authorize_bootstrap(
            &TokenVerifier(&authorization.token),
            authorization.token.expose().as_bytes(),
        )
        .map_err(|_| IpcError::Unauthorized)?;
        let bootstrap = bootstrap_request(&authorization, &request, now)?;
        let proof = ProvisioningProof {
            authorization_id: &authorization.authorization_id,
            presented_digest: &authorization.token.digest().hex(),
            now_epoch: now.get(),
        };

        let unit_of_work = SqliteUnitOfWork::new(product.pool.clone());
        let mut transaction = unit_of_work
            .begin()
            .await
            .map_err(|_| IpcError::InternalError)?;
        provision_initial_installation_with_proof(
            &mut transaction,
            &capability,
            &bootstrap,
            &proof,
            occurred_at.as_str(),
        )
        .await
        .map_err(IpcError::from)?;
        transaction
            .commit()
            .await
            .map_err(|_| IpcError::InternalError)?;

        // El derecho ya está consumido; retirar el archivo reduce la exposición.
        let _ = std::fs::remove_file(product.directory.join(NATIVE_AUTHORIZATION_FILE));
        Ok(())
    }
}

fn status(state: &str, expires_at: Option<u32>) -> BootstrapStatusDto {
    BootstrapStatusDto {
        state: state.to_owned(),
        expires_at,
    }
}

async fn needs_bootstrap(product: &ProductInstallation) -> Result<bool, IpcError> {
    let unit_of_work = SqliteUnitOfWork::new(product.pool.clone());
    let mut transaction = unit_of_work
        .begin()
        .await
        .map_err(|_| IpcError::InternalError)?;
    let needs = requires_provisioning(&mut transaction).await;
    let rolled_back = transaction.rollback().await;
    if rolled_back.is_err() {
        return Err(IpcError::InternalError);
    }
    needs.map_err(|_| IpcError::InternalError)
}

fn file_error(error: AuthorizationFileError) -> IpcError {
    match error {
        AuthorizationFileError::UnsupportedPlatform => IpcError::OperationNotAvailable,
        AuthorizationFileError::Insecure => IpcError::Forbidden,
        AuthorizationFileError::Missing | AuthorizationFileError::Invalid => IpcError::Unauthorized,
    }
}

fn bootstrap_request(
    authorization: &NativeAuthorization,
    request: &BootstrapInstallRequestDto,
    now: UtcSeconds,
) -> Result<BootstrapRequest, IpcError> {
    let name = DisplayName::new(&request.business_name).map_err(|_| IpcError::ValidationError)?;
    let code = match &request.business_code {
        Some(value) => BusinessCode::new(value).map_err(|_| IpcError::ValidationError)?,
        None => BusinessCode::new(&slug(&request.business_name))
            .map_err(|_| IpcError::ValidationError)?,
    };
    let administrator_name =
        DisplayName::new(&request.administrator_name).map_err(|_| IpcError::ValidationError)?;
    let email = NormalizedEmail::new(&request.email).map_err(|_| IpcError::ValidationError)?;
    let currency = IsoCurrency::new(request.currency.as_deref().unwrap_or("MXN"))
        .map_err(|_| IpcError::ValidationError)?;
    let organization_id = uuid_id()?;
    let administrator_id = uuid_id()?;
    let role_id = uuid_id()?;
    let event_id = uuid_id()?;

    let initial_credential = CandidateBootstrapPasswords::new(Argon2Policy::LAB_V1)
        .prepare(
            administrator_id.clone(),
            administrator_id.clone(),
            &request.password,
            now,
        )
        .map_err(|_| IpcError::ValidationError)?;

    Ok(BootstrapRequest {
        installation_id: authorization.installation_id.clone(),
        audit_event_id: AuditEventId::new(event_id),
        organization_id,
        organization: NewOrganization {
            name,
            code,
            timezone: Timezone::new("UTC").map_err(|_| IpcError::ValidationError)?,
            currency,
            locale: LocaleTag::new("es-MX").map_err(|_| IpcError::ValidationError)?,
        },
        administrator_id,
        administrator_name,
        administrator_email: email,
        administrator_role_id: role_id,
        administrator_role_code: BusinessCode::new("administrator")
            .map_err(|_| IpcError::ValidationError)?,
        installation_fingerprint: format!("local:{}", authorization.installation_id.as_str()),
        initial_credential,
    })
}

/// Deriva un slug estable del nombre del negocio.
///
/// El código se persiste y no se modifica después; el wizard permite indicarlo
/// explícitamente cuando el slug automático no sirve.
fn slug(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut pending_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(character.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
        if slug.len() >= 64 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "negocio".to_owned()
    } else {
        slug
    }
}

fn uuid_id() -> Result<EntityId, IpcError> {
    let value = random_uuid_v4().map_err(|_| IpcError::InternalError)?;
    EntityId::parse(&value).map_err(|_| IpcError::InternalError)
}

/// Verificador que acredita posesión del token leído del archivo privado.
struct TokenVerifier<'a>(&'a ProvisioningToken);

impl BootstrapSecretVerifier for TokenVerifier<'_> {
    fn verify(&self, presented: &[u8]) -> Result<(), BootstrapError> {
        let expected = self.0.expose().as_bytes();
        if presented.len() != expected.len() {
            return Err(BootstrapError::Rejected);
        }
        let difference = presented
            .iter()
            .zip(expected)
            .fold(0_u8, |accumulator, (left, right)| {
                accumulator | (left ^ right)
            });
        if difference == 0 {
            Ok(())
        } else {
            Err(BootstrapError::Rejected)
        }
    }
}
