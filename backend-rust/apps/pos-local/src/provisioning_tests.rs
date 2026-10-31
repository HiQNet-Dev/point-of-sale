//! Pruebas del emisor nativo y del recorrido completo sobre el esquema productivo.

#![cfg(unix)]
#![allow(clippy::panic)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

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

async fn candidate_directory()
-> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
    let database = temp.path().join(DATABASE_FILE);
    let pool = open(&database, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    pool.close().await;
    Ok((temp, database))
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
    let (temp, database) = candidate_directory().await?;
    execute(ProvisioningCommand::Issue {
        directory: temp.path().to_path_buf(),
        ttl_seconds: 600,
    })
    .await?;

    let file = temp.path().join(AUTHORIZATION_FILE);
    let metadata = std::fs::symlink_metadata(&file)?;
    assert_eq!(metadata.permissions().mode() & 0o077, 0);
    let fields = read_file(temp.path())?;
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

    let pool = open(&database, Duration::from_millis(75)).await?;
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
async fn issue_refuses_an_insecure_directory_a_second_file_and_a_missing_database()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, _database) = candidate_directory().await?;
    execute(ProvisioningCommand::Issue {
        directory: temp.path().to_path_buf(),
        ttl_seconds: 600,
    })
    .await?;
    assert_eq!(
        execute(ProvisioningCommand::Issue {
            directory: temp.path().to_path_buf(),
            ttl_seconds: 600,
        })
        .await,
        Err(crate::provisioning::ProvisioningError::AlreadyExists)
    );

    let loose = tempfile::tempdir()?;
    std::fs::set_permissions(loose.path(), std::fs::Permissions::from_mode(0o755))?;
    assert_eq!(
        execute(ProvisioningCommand::Issue {
            directory: loose.path().to_path_buf(),
            ttl_seconds: 600,
        })
        .await,
        Err(crate::provisioning::ProvisioningError::InsecureDirectory)
    );

    let empty = tempfile::tempdir()?;
    std::fs::set_permissions(empty.path(), std::fs::Permissions::from_mode(0o700))?;
    assert_eq!(
        execute(ProvisioningCommand::Issue {
            directory: empty.path().to_path_buf(),
            ttl_seconds: 600,
        })
        .await,
        Err(crate::provisioning::ProvisioningError::MissingDatabase)
    );
    Ok(())
}

#[tokio::test]
async fn status_and_revoke_report_state_without_the_token() -> Result<(), Box<dyn std::error::Error>>
{
    let (temp, database) = candidate_directory().await?;
    execute(ProvisioningCommand::Issue {
        directory: temp.path().to_path_buf(),
        ttl_seconds: 600,
    })
    .await?;
    execute(ProvisioningCommand::Status {
        directory: temp.path().to_path_buf(),
    })
    .await?;
    execute(ProvisioningCommand::Revoke {
        directory: temp.path().to_path_buf(),
    })
    .await?;

    let fields = read_file(temp.path())?;
    let authorization_id = fields
        .iter()
        .find(|(key, _)| key == "authorization_id")
        .map(|(_, value)| value)
        .ok_or("authorization_id")?;
    let pool = open(&database, Duration::from_millis(75)).await?;
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
    let (temp, database) = candidate_directory().await?;
    execute(ProvisioningCommand::Issue {
        directory: temp.path().to_path_buf(),
        ttl_seconds: 600,
    })
    .await?;
    let fields = read_file(temp.path())?;
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
