//! Recorrido integral del provisioning nativo sobre el esquema **candidato**.
//!
//! Emite una autorización con el tipo criptográfico real, la persiste con
//! `SqliteProvisioningStore`, ejecuta el bootstrap exigiendo su consumo en la
//! misma transacción, reabre la base y comprueba que el token no puede reusarse.
//! No hay red, Java ni servicio HTTP; `0017` sigue fuera del migrador productivo.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_application::bootstrap::{
    BootstrapRequest, ProvisioningProof, provision_initial_installation_with_proof,
};
use pos_core::{
    audit::{AuditEventId, BootstrapProvisioning, BootstrapSecretVerifier, authorize_bootstrap},
    identity::{
        NormalizedEmail, ProvisioningAuthorization, ProvisioningPurpose, ProvisioningState,
    },
    organization_branch::{
        BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewOrganization, Timezone,
    },
};
use pos_credentials::ProvisioningToken;
use sqlx::SqlitePool;

use super::*;
use crate::provisioning::SqliteProvisioningStore;

const PROPOSALS: [&str; 7] = [
    include_str!("../../../proposals/local-auth/0002_installation_identity.proposed.sql"),
    include_str!("../../../proposals/local-auth/0003_user_credential_session.proposed.sql"),
    include_str!("../../../proposals/local-auth/0004_rbac_catalog.proposed.sql"),
    include_str!("../../../proposals/local-auth/0005_branch_membership.proposed.sql"),
    include_str!("../../../proposals/local-auth/0006_audit_event.proposed.sql"),
    include_str!("../../../proposals/local-auth/0007_bootstrap_record.proposed.sql"),
    include_str!("../../../proposals/local-auth/0017_bootstrap_authorization.proposed.sql"),
];

const ORG: &str = "00000000-0000-0000-0000-0000000000a0";
const INSTALLATION: &str = "00000000-0000-0000-0000-0000000000f0";
const ADMIN: &str = "00000000-0000-0000-0000-0000000000a1";
const ROLE: &str = "00000000-0000-0000-0000-0000000000b0";
const BOOTSTRAP_EVENT: &str = "00000000-0000-0000-0000-0000000000e0";
const AUTHORIZATION: &str = "00000000-0000-0000-0000-0000000001a0";
const ISSUED_EVENT: &str = "00000000-0000-0000-0000-0000000001e0";
const REVOKE_EVENT: &str = "00000000-0000-0000-0000-0000000001e1";
const TIME: &str = "2026-10-29T00:00:00Z";
const NOW: i64 = 1_793_232_000;

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
        .unwrap_or_else(|_| panic!("capacidad de laboratorio"))
}

async fn open_candidate(path: &std::path::Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = open(path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    for statement in PROPOSALS {
        sqlx::raw_sql(statement).execute(&pool).await?;
    }
    sqlx::query(
        "INSERT INTO provisional_installation (id, fingerprint, created_at) \
         VALUES (?, 'ed25519:native-test', ?)",
    )
    .bind(INSTALLATION)
    .bind(NOW)
    .execute(&pool)
    .await?;
    Ok(pool)
}

fn request() -> BootstrapRequest {
    let administrator = id(ADMIN);
    BootstrapRequest {
        installation_id: id(INSTALLATION),
        audit_event_id: AuditEventId::new(id(BOOTSTRAP_EVENT)),
        organization_id: id(ORG),
        organization: NewOrganization {
            name: DisplayName::new("Organización Inicial")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("org-inicial").unwrap_or_else(|error| panic!("code: {error}")),
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
        installation_fingerprint: "ed25519:native-test".to_owned(),
        initial_credential: crate::bootstrap::CandidateBootstrapPasswords::new(
            pos_credentials::Argon2Policy::LAB_V1,
        )
        .prepare(
            administrator.clone(),
            administrator,
            "contraseña-sintetica-nativa-2k",
            pos_core::identity::UtcSeconds::new(NOW)
                .unwrap_or_else(|error| panic!("instante: {error}")),
        )
        .unwrap_or_else(|_| panic!("credencial sintética")),
    }
}

async fn issue(
    store: &SqliteProvisioningStore,
    token: &ProvisioningToken,
    issued: i64,
    expires: i64,
) {
    let authorization = ProvisioningAuthorization::issue(
        id(AUTHORIZATION),
        id(INSTALLATION),
        ProvisioningPurpose::Bootstrap,
        &token.digest().hex(),
        issued,
        expires,
    )
    .unwrap_or_else(|error| panic!("autorización: {error}"));
    store
        .issue(&authorization, &id(ISSUED_EVENT), issued)
        .await
        .unwrap_or_else(|error| panic!("emisión: {error}"));
}

#[tokio::test]
async fn native_journey_consumes_the_token_in_the_bootstrap_transaction()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("native.sqlite");
    let pool = open_candidate(&path).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;
    issue(&store, &token, NOW, NOW + 600).await;

    let request = request();
    let authorization_id = id(AUTHORIZATION);
    let presented = token.digest().hex();
    let proof = ProvisioningProof {
        authorization_id: &authorization_id,
        presented_digest: &presented,
        now_epoch: NOW + 30,
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

    // Efectos: organización, instalación canónica, bootstrap y consumo.
    let organizations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organization")
        .fetch_one(&pool)
        .await?;
    assert_eq!(organizations, 1);
    let installations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM installation_identity")
        .fetch_one(&pool)
        .await?;
    assert_eq!(installations, 1);
    let bootstrap_state: String =
        sqlx::query_scalar("SELECT state FROM bootstrap_record WHERE installation_id = ?")
            .bind(INSTALLATION)
            .fetch_one(&pool)
            .await?;
    assert_eq!(bootstrap_state, "COMPLETED");
    assert_eq!(
        store
            .get(&id(AUTHORIZATION))
            .await?
            .map(|value| value.state()),
        Some(ProvisioningState::Consumed)
    );
    let claim: i64 = sqlx::query_scalar(
        "SELECT claimed_installation_id IS NOT NULL FROM provisional_installation WHERE id = ?",
    )
    .bind(INSTALLATION)
    .fetch_one(&pool)
    .await?;
    assert_eq!(claim, 1);

    // Replay: el mismo token no vuelve a provisionar.
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

    // Reapertura: identidad, bootstrap y consumo persisten.
    pool.close().await;
    let reopened = open(&path, Duration::from_millis(75)).await?;
    let store = SqliteProvisioningStore::new(reopened.clone());
    assert_eq!(
        store
            .get(&id(AUTHORIZATION))
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

async fn rejection(scenario: &str) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open_candidate(&temp.path().join(format!("{scenario}.sqlite"))).await?;
    let store = SqliteProvisioningStore::new(pool.clone());
    let token = ProvisioningToken::generate()?;
    issue(&store, &token, NOW, NOW + 600).await;

    let (presented, now_epoch) = match scenario {
        "wrong" => (ProvisioningToken::generate()?.digest().hex(), NOW + 30),
        "expired" => (token.digest().hex(), NOW + 601),
        _ => {
            store
                .revoke(&id(AUTHORIZATION), &id(REVOKE_EVENT), NOW + 10)
                .await?;
            (token.digest().hex(), NOW + 30)
        }
    };

    let request = request();
    let authorization_id = id(AUTHORIZATION);
    let proof = ProvisioningProof {
        authorization_id: &authorization_id,
        presented_digest: &presented,
        now_epoch,
    };
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
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
        .is_err(),
        "escenario {scenario}"
    );
    transaction.rollback().await?;

    let organizations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organization")
        .fetch_one(&pool)
        .await?;
    assert_eq!(organizations, 0, "escenario {scenario}");
    assert_ne!(
        store
            .get(&id(AUTHORIZATION))
            .await?
            .map(|value| value.state()),
        Some(ProvisioningState::Consumed),
        "escenario {scenario}"
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn a_wrong_token_never_provisions() -> Result<(), Box<dyn std::error::Error>> {
    rejection("wrong").await
}

#[tokio::test]
async fn an_expired_token_never_provisions() -> Result<(), Box<dyn std::error::Error>> {
    rejection("expired").await
}

#[tokio::test]
async fn a_revoked_token_never_provisions() -> Result<(), Box<dyn std::error::Error>> {
    rejection("revoked").await
}
