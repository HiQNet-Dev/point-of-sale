//! Pruebas del emisor nativo y del recorrido completo sobre el esquema productivo.
//!
//! Se ejecutan en Unix y en Windows: la custodia del archivo entregado la
//! resuelve cada plataforma, pero el contrato (archivo privado, consumo único,
//! estado del derecho) es el mismo.

#![cfg(any(unix, windows))]
#![allow(clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use pos_application::bootstrap::{
    BootstrapRequest, ProvisioningProof, provision_initial_installation_with_proof,
};
use pos_core::{
    audit::{AuditEventId, BootstrapProvisioning, BootstrapSecretVerifier, authorize_bootstrap},
    identity::{NormalizedEmail, ProvisioningPurpose, ProvisioningState, UtcSeconds},
    organization_branch::{
        BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewOrganization, Timezone,
    },
};
use pos_credentials::ProvisioningToken;
use pos_storage_sqlite::{SqliteUnitOfWork, migrate, open, provisioning::SqliteProvisioningStore};

use crate::provisioning::{AUTHORIZATION_FILE, DATABASE_FILE, ProvisioningCommand, execute};

const ORG: &str = "00000000-0000-0000-0000-0000000000a0";
const ADMIN: &str = "00000000-0000-0000-0000-0000000000a1";
const ROLE: &str = "00000000-0000-0000-0000-0000000000b0";
const BOOTSTRAP_EVENT: &str = "00000000-0000-0000-0000-0000000000e0";
const TIME: &str = "2026-10-29T00:00:00Z";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

struct LabAuthority;
impl BootstrapSecretVerifier for LabAuthority {
    fn verify(&self, presented: &[u8]) -> Result<(), pos_core::audit::BootstrapError> {
        if presented == b"capability-laboratorio" {
            Ok(())
        } else {
            Err(pos_core::audit::BootstrapError::Rejected)
        }
    }
}

fn capability() -> BootstrapProvisioning {
    authorize_bootstrap(&LabAuthority, b"capability-laboratorio")
        .unwrap_or_else(|_| panic!("capacidad"))
}

/// Directorio de instalación con base migrada, listo para emitir.
async fn candidate_directory() -> Result<(tempfile::TempDir, PathBuf), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let directory = prepare_candidate_directory(temp.path())?;
    let database = directory.join(DATABASE_FILE);
    let pool = open(&database, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    pool.close().await;
    Ok((temp, directory))
}

#[cfg(unix)]
fn prepare_candidate_directory(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
    Ok(root.to_path_buf())
}

#[cfg(windows)]
fn prepare_candidate_directory(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let directory = root.join("com.hiqnet.pos");
    pos_windows_security::create_private_directory(&directory)?;
    Ok(directory)
}

/// Deja el directorio en un estado que el emisor debe rechazar.
#[cfg(unix)]
fn make_directory_shared(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// Concede control total a `Everyone` mediante su SID, sin depender del idioma.
#[cfg(windows)]
fn make_directory_shared(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("icacls")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(OI)(CI)F"])
        .status()?;
    if !status.success() {
        return Err("icacls no pudo ampliar la DACL".into());
    }
    Ok(())
}

/// Comprueba la custodia del archivo entregado con la regla de la plataforma.
#[cfg(unix)]
fn assert_authorization_is_private(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = std::fs::symlink_metadata(directory.join(AUTHORIZATION_FILE))?;
    assert_eq!(metadata.permissions().mode() & 0o077, 0);
    Ok(())
}

#[cfg(windows)]
fn assert_authorization_is_private(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    pos_windows_security::verify_private_file(directory, AUTHORIZATION_FILE)?;
    Ok(())
}

/// Instante real del recorrido; la autorización se emite con el reloj de
/// pared, así que un valor congelado en el pasado haría fallar el consumo.
fn now_epoch() -> i64 {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("reloj: {error}"));
    i64::try_from(elapsed.as_secs()).unwrap_or_else(|error| panic!("instante: {error}"))
}

fn read_file(directory: &Path) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(directory.join(AUTHORIZATION_FILE))?;
    Ok(contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}

#[tokio::test]
async fn issue_delivers_a_private_file_and_a_pending_authorization()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, directory) = candidate_directory().await?;
    execute(ProvisioningCommand::Issue {
        directory: directory.clone(),
        ttl_seconds: 600,
    })
    .await?;

    assert_authorization_is_private(&directory)?;
    let fields = read_file(&directory)?;
    let token = fields
        .iter()
        .find(|(key, _)| key == "token")
        .map(|(_, value)| value)
        .ok_or("token")?;
    assert_eq!(token.len(), 64);
    let authorization_id = fields
        .iter()
        .find(|(key, _)| key == "authorization_id")
        .map(|(_, value)| value)
        .ok_or("authorization_id")?;

    let pool = open(&directory.join(DATABASE_FILE), Duration::from_millis(75)).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let stored = store
        .get(&EntityId::parse(authorization_id)?)
        .await?
        .ok_or("autorización persistida")?;
    assert_eq!(stored.state(), ProvisioningState::Pending);
    assert_eq!(stored.purpose(), ProvisioningPurpose::Bootstrap);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn issue_refuses_a_second_file_and_a_missing_database()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, directory) = candidate_directory().await?;
    execute(ProvisioningCommand::Issue {
        directory: directory.clone(),
        ttl_seconds: 600,
    })
    .await?;
    assert_eq!(
        execute(ProvisioningCommand::Issue {
            directory: directory.clone(),
            ttl_seconds: 600,
        })
        .await,
        Err(crate::provisioning::ProvisioningError::AlreadyExists)
    );

    let empty = tempfile::tempdir()?;
    let empty = prepare_candidate_directory(empty.path())?;
    assert_eq!(
        execute(ProvisioningCommand::Issue {
            directory: empty,
            ttl_seconds: 600,
        })
        .await,
        Err(crate::provisioning::ProvisioningError::MissingDatabase)
    );
    Ok(())
}

#[tokio::test]
async fn issue_refuses_an_insecure_directory() -> Result<(), Box<dyn std::error::Error>> {
    let (temp, directory) = candidate_directory().await?;
    let shared = temp.path().join("compartida");
    std::fs::create_dir(&shared)?;
    make_directory_shared(&shared)?;
    assert_eq!(
        execute(ProvisioningCommand::Issue {
            directory: shared.clone(),
            ttl_seconds: 600,
        })
        .await,
        Err(crate::provisioning::ProvisioningError::InsecureDirectory)
    );
    // Ninguna entrega parcial debe quedar en el directorio compartido.
    assert!(!shared.join(AUTHORIZATION_FILE).exists());
    assert!(directory.join(DATABASE_FILE).exists());
    Ok(())
}

#[tokio::test]
async fn status_and_revoke_report_state_without_the_token() -> Result<(), Box<dyn std::error::Error>>
{
    let (_temp, directory) = candidate_directory().await?;
    execute(ProvisioningCommand::Issue {
        directory: directory.clone(),
        ttl_seconds: 600,
    })
    .await?;
    execute(ProvisioningCommand::Status {
        directory: directory.clone(),
    })
    .await?;
    execute(ProvisioningCommand::Revoke {
        directory: directory.clone(),
    })
    .await?;

    let fields = read_file(&directory)?;
    let authorization_id = fields
        .iter()
        .find(|(key, _)| key == "authorization_id")
        .map(|(_, value)| value)
        .ok_or("authorization_id")?;
    let pool = open(&directory.join(DATABASE_FILE), Duration::from_millis(75)).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    assert_eq!(
        store
            .get(&EntityId::parse(authorization_id)?)
            .await?
            .map(|value| value.state()),
        Some(ProvisioningState::Revoked)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn the_issued_token_provisions_once_and_cannot_be_replayed()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, directory) = candidate_directory().await?;
    let database = directory.join(DATABASE_FILE);
    execute(ProvisioningCommand::Issue {
        directory: directory.clone(),
        ttl_seconds: 600,
    })
    .await?;
    let fields = read_file(&directory)?;
    let value = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("campo {name}"))
    };
    let installation_id = EntityId::parse(&value("installation_id"))?;
    let authorization_id = EntityId::parse(&value("authorization_id"))?;
    let token = ProvisioningToken::from_encoded(&value("token"))?;
    let instant = now_epoch();

    let pool = open(&database, Duration::from_millis(75)).await?;
    let administrator = id(ADMIN);
    let request = BootstrapRequest {
        installation_id: installation_id.clone(),
        audit_event_id: AuditEventId::new(id(BOOTSTRAP_EVENT)),
        organization_id: id(ORG),
        organization: NewOrganization {
            name: DisplayName::new("Organización Nativa")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("org-nativa").unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
        administrator_id: administrator.clone(),
        administrator_name: DisplayName::new("Administradora")
            .unwrap_or_else(|error| panic!("name: {error}")),
        administrator_email: NormalizedEmail::new("admin@hiqnet.test")
            .unwrap_or_else(|error| panic!("email: {error}")),
        administrator_role_id: id(ROLE),
        administrator_role_code: BusinessCode::new("administrator")
            .unwrap_or_else(|error| panic!("rol: {error}")),
        installation_fingerprint: format!("local:{installation_id}"),
        initial_credential: pos_storage_sqlite::bootstrap::CandidateBootstrapPasswords::new(
            pos_credentials::Argon2Policy::LAB_V1,
        )
        .prepare(
            administrator.clone(),
            administrator,
            "contraseña-sintetica-cli-2k",
            UtcSeconds::new(instant).unwrap_or_else(|error| panic!("instante: {error}")),
        )
        .unwrap_or_else(|_| panic!("credencial sintética")),
    };
    let digest = token.digest().hex();
    let proof = ProvisioningProof {
        authorization_id: &authorization_id,
        presented_digest: &digest,
        now_epoch: instant,
    };

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    provision_initial_installation_with_proof(
        &mut transaction,
        &capability(),
        &request,
        &proof,
        TIME,
    )
    .await?;
    transaction.commit().await?;

    let mut transaction = unit_of_work.begin().await?;
    assert!(
        provision_initial_installation_with_proof(
            &mut transaction,
            &capability(),
            &request,
            &proof,
            TIME,
        )
        .await
        .is_err()
    );
    transaction.rollback().await?;

    pool.close().await;
    let reopened = open(&database, Duration::from_millis(75)).await?;
    let store = SqliteProvisioningStore::new(reopened.clone());
    assert_eq!(
        store
            .get(&authorization_id)
            .await?
            .map(|value| value.state()),
        Some(ProvisioningState::Consumed)
    );
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pos_user")
        .fetch_one(&reopened)
        .await?;
    assert_eq!(users, 1);
    reopened.close().await;
    Ok(())
}
