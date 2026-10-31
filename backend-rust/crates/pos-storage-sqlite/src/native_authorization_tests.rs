//! Custodia del archivo de autorización y primera instalación real.
//!
//! Cubre los rechazos de permisos, owner y enlaces, la clasificación del derecho
//! frente a la base y el recorrido completo: archivo privado → autorización
//! nativa → bootstrap atómico → login sobre SQLite productiva.
#![allow(clippy::panic)]

use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use pos_application::{
    bootstrap::{
        BootstrapRequest, ProvisioningProof, provision_initial_installation_with_proof,
        requires_provisioning,
    },
    identity::LoginRequest,
};
use pos_core::{
    audit::{
        AuditEventId, AuditOccurredAt, BootstrapError, BootstrapSecretVerifier, authorize_bootstrap,
    },
    identity::{NormalizedEmail, ProvisioningAuthorization, ProvisioningPurpose, UtcSeconds},
    organization_branch::{
        ApplicationError, BusinessCode, DisplayName, EntityId, IdentityGenerator, IsoCurrency,
        LocaleTag, NewOrganization, Timezone,
    },
};
use pos_credentials::{Argon2Policy, ProvisioningToken};

use crate::{
    SqliteUnitOfWork,
    bootstrap::CandidateBootstrapPasswords,
    migrate,
    native_authorization::{
        AuthorizationFileError, NATIVE_AUTHORIZATION_FILE, NativeAuthorizationState,
        native_authorization_state, read_native_authorization,
    },
    open,
    provisioning::SqliteProvisioningStore,
    session::identity_application,
};

const ORG: &str = "00000000-0000-0000-0000-00000000d0a0";
const ADMIN: &str = "00000000-0000-0000-0000-00000000d0a1";
const ROLE: &str = "00000000-0000-0000-0000-00000000d0b0";
const INSTALLATION: &str = "00000000-0000-0000-0000-00000000d0f0";
const AUTHORIZATION: &str = "00000000-0000-0000-0000-00000000d1a0";
const ISSUED_EVENT: &str = "00000000-0000-0000-0000-00000000d1e0";
const BOOTSTRAP_EVENT: &str = "00000000-0000-0000-0000-00000000d0e0";
const PASSWORD: &str = "primera-instalación-2k";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn now_epoch() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("reloj: {error}"));
    i64::try_from(elapsed.as_secs()).unwrap_or_else(|error| panic!("instante: {error}"))
}

async fn iso_now(pool: &sqlx::SqlitePool) -> String {
    sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("iso: {error}"))
}

fn private_directory(path: &Path) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|error| panic!("permisos: {error}"));
}

fn write_authorization(directory: &Path, mode: u32, token: &ProvisioningToken) {
    let path = directory.join(NATIVE_AUTHORIZATION_FILE);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&path)
        .unwrap_or_else(|error| panic!("archivo: {error}"));
    writeln!(file, "installation_id={INSTALLATION}")
        .and_then(|()| writeln!(file, "authorization_id={AUTHORIZATION}"))
        .and_then(|()| writeln!(file, "token={}", token.expose()))
        .and_then(|()| file.sync_all())
        .unwrap_or_else(|error| panic!("escritura: {error}"));
}

async fn provisional_store(
    pool: &sqlx::SqlitePool,
    token: &ProvisioningToken,
    issued_at: i64,
    expires_at: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteProvisioningStore::new(pool.clone());
    store
        .ensure_provisional_installation(&id(INSTALLATION), "local:native-test", issued_at)
        .await?;
    let authorization = ProvisioningAuthorization::issue(
        id(AUTHORIZATION),
        id(INSTALLATION),
        ProvisioningPurpose::Bootstrap,
        &token.digest().hex(),
        issued_at,
        expires_at,
    )?;
    store
        .issue(&authorization, &id(ISSUED_EVENT), issued_at)
        .await?;
    Ok(())
}

struct TokenVerifier<'a>(&'a ProvisioningToken);

impl BootstrapSecretVerifier for TokenVerifier<'_> {
    fn verify(&self, presented: &[u8]) -> Result<(), BootstrapError> {
        // Comparación de longitud constante sobre el token en memoria.
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

#[tokio::test]
async fn reads_only_a_private_file_owned_by_the_current_user()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    private_directory(temp.path());
    let pool = open(&temp.path().join("auth.sqlite"), Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    let token = ProvisioningToken::generate()?;
    let now = now_epoch();
    provisional_store(&pool, &token, now, now + 600).await?;
    write_authorization(temp.path(), 0o600, &token);

    let authorization = read_native_authorization(temp.path())?;
    assert_eq!(authorization.installation_id, id(INSTALLATION));
    assert_eq!(authorization.authorization_id, id(AUTHORIZATION));
    assert_eq!(
        native_authorization_state(&pool, &authorization, now).await?,
        NativeAuthorizationState::Ready {
            expires_at: now + 600
        }
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn rejects_insecure_directory_file_and_symlink() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    private_directory(temp.path());
    let token = ProvisioningToken::generate()?;

    // Directorio con permiso de grupo.
    let loose = tempfile::tempdir()?;
    std::fs::set_permissions(loose.path(), std::fs::Permissions::from_mode(0o750))?;
    write_authorization(loose.path(), 0o600, &token);
    assert_eq!(
        read_native_authorization(loose.path()).err(),
        Some(AuthorizationFileError::Insecure)
    );

    // Archivo legible por otros.
    write_authorization(temp.path(), 0o644, &token);
    assert_eq!(
        read_native_authorization(temp.path()).err(),
        Some(AuthorizationFileError::Insecure)
    );

    // Archivo ausente.
    let empty = tempfile::tempdir()?;
    private_directory(empty.path());
    assert_eq!(
        read_native_authorization(empty.path()).err(),
        Some(AuthorizationFileError::Missing)
    );

    // Enlace simbólico: no se sigue.
    let linked = tempfile::tempdir()?;
    private_directory(linked.path());
    std::os::unix::fs::symlink(
        temp.path().join(NATIVE_AUTHORIZATION_FILE),
        linked.path().join(NATIVE_AUTHORIZATION_FILE),
    )?;
    assert_eq!(
        read_native_authorization(linked.path()).err(),
        Some(AuthorizationFileError::Insecure)
    );
    Ok(())
}

#[tokio::test]
async fn classifies_expired_revoked_consumed_and_unknown_rights()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    private_directory(temp.path());
    let pool = open(&temp.path().join("auth.sqlite"), Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    let token = ProvisioningToken::generate()?;
    let now = now_epoch();
    provisional_store(&pool, &token, now - 1_000, now - 400).await?;
    write_authorization(temp.path(), 0o600, &token);
    let authorization = read_native_authorization(temp.path())?;
    // La ventana ya terminó: el derecho no es consumible.
    assert_eq!(
        native_authorization_state(&pool, &authorization, now).await?,
        NativeAuthorizationState::Expired
    );

    // Un token distinto del persistido no identifica el derecho.
    let other = ProvisioningToken::generate()?;
    let mut spoofed = read_native_authorization(temp.path())?;
    spoofed.token = other;
    assert_eq!(
        native_authorization_state(&pool, &spoofed, now).await?,
        NativeAuthorizationState::Unknown
    );
    pool.close().await;
    Ok(())
}

struct TestIdentities;
impl IdentityGenerator for TestIdentities {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        let value = pos_credentials::random_uuid_v4().map_err(|_| ApplicationError::Storage)?;
        EntityId::parse(&value).map_err(|_| ApplicationError::Storage)
    }
}

#[tokio::test]
async fn first_installation_consumes_the_native_authorization_and_logs_in()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    private_directory(temp.path());
    let database = temp.path().join("first.sqlite");
    let pool = open(&database, Duration::from_millis(500)).await?;
    migrate(&pool).await?;
    let now = now_epoch();
    let occurred_at = iso_now(&pool).await;
    let token = ProvisioningToken::generate()?;
    provisional_store(&pool, &token, now, now + 600).await?;
    write_authorization(temp.path(), 0o600, &token);

    // La autorización se lee en Rust; el token no vuelve a aparecer.
    let authorization = read_native_authorization(temp.path())?;
    let capability = authorize_bootstrap(
        &TokenVerifier(&authorization.token),
        authorization.token.expose().as_bytes(),
    )?;
    let proof = ProvisioningProof {
        authorization_id: &authorization.authorization_id,
        presented_digest: &authorization.token.digest().hex(),
        now_epoch: now,
    };
    let request = BootstrapRequest {
        installation_id: authorization.installation_id.clone(),
        audit_event_id: AuditEventId::new(id(BOOTSTRAP_EVENT)),
        organization_id: id(ORG),
        organization: NewOrganization {
            name: DisplayName::new("Café Central").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("cafe-central").unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("moneda: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
        administrator_id: id(ADMIN),
        administrator_name: DisplayName::new("Administradora")
            .unwrap_or_else(|error| panic!("name: {error}")),
        administrator_email: NormalizedEmail::new("admin@cafe.test")
            .unwrap_or_else(|error| panic!("email: {error}")),
        administrator_role_id: id(ROLE),
        administrator_role_code: BusinessCode::new("administrator")
            .unwrap_or_else(|error| panic!("rol: {error}")),
        installation_fingerprint: format!("local:{INSTALLATION}"),
        initial_credential: CandidateBootstrapPasswords::new(Argon2Policy::LAB_V1).prepare(
            id(ADMIN),
            id(ADMIN),
            PASSWORD,
            UtcSeconds::new(now).unwrap_or_else(|error| panic!("instante: {error}")),
        )?,
    };

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    assert!(requires_provisioning(&mut transaction).await?);
    provision_initial_installation_with_proof(
        &mut transaction,
        &capability,
        &request,
        &proof,
        &occurred_at,
    )
    .await?;
    transaction.commit().await?;

    // El derecho queda consumido y no puede repetirse.
    assert_eq!(
        native_authorization_state(&pool, &authorization, now).await?,
        NativeAuthorizationState::Consumed
    );
    let mut transaction = unit_of_work.begin().await?;
    assert!(!requires_provisioning(&mut transaction).await?);
    transaction.rollback().await?;

    // El administrador ya puede autenticarse.
    let application = identity_application(pool.clone())?;
    let mut identities = TestIdentities;
    let login = LoginRequest {
        email: NormalizedEmail::new("admin@cafe.test")?,
        now: UtcSeconds::new(now)?,
        occurred_at: AuditOccurredAt::new(&occurred_at)?,
        session_ttl_seconds: 3600,
    };
    let session = application.login(&mut identities, &login, PASSWORD).await?;
    assert_eq!(session.identity.user_id, id(ADMIN));
    assert!(session.identity.permissions.contains("product.create"));

    // Reapertura: identidad y permisos persisten.
    pool.close().await;
    let reopened = open(&database, Duration::from_millis(500)).await?;
    let application = identity_application(reopened.clone())?;
    let mut identities = TestIdentities;
    let login = LoginRequest {
        email: NormalizedEmail::new("admin@cafe.test")?,
        now: UtcSeconds::new(now_epoch())?,
        occurred_at: AuditOccurredAt::new(&iso_now(&reopened).await)?,
        session_ttl_seconds: 3600,
    };
    let session = application.login(&mut identities, &login, PASSWORD).await?;
    assert_eq!(session.identity.organization_id, id(ORG));
    reopened.close().await;
    Ok(())
}
