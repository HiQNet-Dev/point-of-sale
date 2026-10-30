//! Flujo administrativo completo sobre el esquema **candidato**.
//!
//! No es producto desplegable: las tablas vienen de `proposals/local-auth/` y
//! `pos-local` no las carga. La prueba demuestra que los contratos de dominio,
//! el adaptador canónico, la autorización DB-authoritative, la auditoría y el
//! CAS de sesión encajan de extremo a extremo sin red.
#![allow(clippy::panic)]

use std::time::Duration;

use pos_core::{
    audit::{
        AuditAction, AuditEntityType, AuditEventId, AuditEventRecord, AuditMetadata,
        AuditOccurredAt, BootstrapError, BootstrapProvisioning, BootstrapSecretVerifier,
        authorize_bootstrap,
    },
    identity::{
        AuthorizationError, LocalPrincipalService, NormalizedEmail, PermissionCode, SessionHash,
        UtcSeconds,
    },
    organization_branch::{
        Branch, BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewBranch,
        NewOrganization, Timezone,
    },
};
use sqlx::SqlitePool;

use super::*;
use crate::{
    audit::SqliteAuditWriter,
    bootstrap::{BootstrapRequest, provision_initial_installation},
    identity::SqliteIdentityLab,
};

const PROPOSALS: [&str; 6] = [
    include_str!("../../../proposals/local-auth/0002_installation_identity.proposed.sql"),
    include_str!("../../../proposals/local-auth/0003_user_credential_session.proposed.sql"),
    include_str!("../../../proposals/local-auth/0004_rbac_catalog.proposed.sql"),
    include_str!("../../../proposals/local-auth/0005_branch_membership.proposed.sql"),
    include_str!("../../../proposals/local-auth/0006_audit_event.proposed.sql"),
    include_str!("../../../proposals/local-auth/0007_bootstrap_record.proposed.sql"),
];

const ORG: &str = "00000000-0000-0000-0000-0000000000a0";
const INSTALLATION: &str = "00000000-0000-0000-0000-0000000000f0";
const ADMIN: &str = "00000000-0000-0000-0000-0000000000a1";
const ROLE: &str = "00000000-0000-0000-0000-0000000000b0";
const BRANCH: &str = "00000000-0000-0000-0000-000000000090";
const SESSION: &str = "00000000-0000-0000-0000-0000000000d0";
const BOOTSTRAP_EVENT: &str = "00000000-0000-0000-0000-0000000000e0";
const BRANCH_EVENT: &str = "00000000-0000-0000-0000-0000000000e1";
const TIME: &str = "2026-10-29T00:00:00Z";
const LATER: &str = "2026-10-30T00:00:00Z";
const HASH: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn utc(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture instante: {error}"))
}
fn permission(value: &str) -> PermissionCode {
    PermissionCode::new(value).unwrap_or_else(|error| panic!("permiso: {error}"))
}

struct TestVerifier;

impl BootstrapSecretVerifier for TestVerifier {
    fn verify(&self, presented: &[u8]) -> Result<(), BootstrapError> {
        if presented == b"secreto-de-prueba" {
            Ok(())
        } else {
            Err(BootstrapError::Rejected)
        }
    }
}

async fn open_candidate() -> Result<(tempfile::TempDir, SqlitePool), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(&temp.path().join("flow.sqlite"), Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    for statement in PROPOSALS {
        sqlx::raw_sql(statement).execute(&pool).await?;
    }
    Ok((temp, pool))
}

fn request() -> BootstrapRequest {
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
        administrator_id: id(ADMIN),
        administrator_name: DisplayName::new("Administradora")
            .unwrap_or_else(|error| panic!("name: {error}")),
        administrator_email: NormalizedEmail::new("admin@hiqnet.test")
            .unwrap_or_else(|error| panic!("email: {error}")),
        administrator_role_id: id(ROLE),
        administrator_role_code: BusinessCode::new("administrator")
            .unwrap_or_else(|error| panic!("rol: {error}")),
        installation_fingerprint: "ed25519:test-fingerprint".to_owned(),
    }
}

#[tokio::test]
async fn administrative_flow_bootstraps_authorizes_audits_and_revokes()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    // Secreto incorrecto: no hay capacidad, no hay bootstrap auditable.
    assert!(authorize_bootstrap(&TestVerifier, b"otro").is_err());
    let capability: BootstrapProvisioning =
        authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")
            .unwrap_or_else(|error| panic!("capacidad: {error}"));

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;

    let organization =
        provision_initial_installation(&mut transaction, &capability, &request(), TIME).await?;
    assert_eq!(organization.id(), &id(ORG));

    // El rol administrador recibe permisos reales del catálogo.
    for code in ["organization.read", "branch.create", "branch.read"] {
        sqlx::query("INSERT INTO role_permission (role_id, permission_code) VALUES (?, ?)")
            .bind(ROLE)
            .bind(code)
            .execute(&mut *transaction.connection())
            .await?;
    }

    // Sucursal creada con el adaptador de Organization/Branch ya existente.
    let branch = Branch::create(
        id(BRANCH),
        id(ORG),
        NewBranch {
            name: DisplayName::new("Sucursal Centro")
                .unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new("centro").unwrap_or_else(|error| panic!("code: {error}")),
            timezone_override: None,
        },
    );
    transaction.insert_branch(&branch, TIME).await?;

    // Sesión sintética de la administradora.
    sqlx::query(
        "INSERT INTO local_session (id, user_id, session_hash, state, created_at, expires_at, revoked_at, session_version) \
         VALUES (?, ?, ?, 'ACTIVE', ?, ?, NULL, 1)",
    )
    .bind(SESSION)
    .bind(ADMIN)
    .bind(HASH)
    .bind(TIME)
    .bind(LATER)
    .execute(&mut *transaction.connection())
    .await?;

    // Principal confiable y autorización DB-authoritative, dentro de la misma UoW.
    let hash = SessionHash::new(HASH).unwrap_or_else(|error| panic!("hash: {error}"));
    let principal =
        LocalPrincipalService::new(SqliteIdentityLab::canonical(transaction.connection()))
            .authenticate(&hash, utc(1_793_235_600), Some(&id(BRANCH)))
            .await
            .unwrap_or_else(|error| panic!("authenticate: {error}"));
    assert_eq!(principal.user_id(), &id(ADMIN));
    assert_eq!(
        principal.authorize(&permission("branch.create"), principal.scope()),
        Ok(())
    );
    assert_eq!(
        principal.authorize(&permission("sale.checkout"), principal.scope()),
        Err(AuthorizationError::PermissionDenied),
        "deny-by-default para un permiso no concedido"
    );

    let event = AuditEventRecord::from_verified_principal(
        AuditEventId::new(id(BRANCH_EVENT)),
        &principal,
        AuditAction::new("branch.created").unwrap_or_else(|error| panic!("acción: {error}")),
        AuditEntityType::new("branch").unwrap_or_else(|error| panic!("tipo: {error}")),
        Some(id(BRANCH)),
        AuditMetadata::empty(),
        AuditOccurredAt::new(TIME).unwrap_or_else(|error| panic!("instante: {error}")),
    );
    SqliteAuditWriter::append(&mut transaction, &event).await?;
    transaction.commit().await?;

    // Estado persistido tras el commit.
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM bootstrap_record WHERE installation_id = ?"
        )
        .bind(INSTALLATION)
        .fetch_one(&pool)
        .await?,
        "COMPLETED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM branch")
            .fetch_one(&pool)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event")
            .fetch_one(&pool)
            .await?,
        2,
        "bootstrap SYSTEM + creación de sucursal USER"
    );

    // Revocación CAS con instante ISO del esquema candidato.
    let mut connection = pool.acquire().await?;
    // El adapter toma prestada la conexión; se libera al salir del bloque.
    {
        let mut adapter = SqliteIdentityLab::canonical(&mut connection);
        assert_eq!(
            adapter
                .revoke_session_cas_at(&principal, &id(SESSION), 1, LATER)
                .await?,
            CasOutcome::Updated
        );
        assert_eq!(
            adapter
                .revoke_session_cas_at(&principal, &id(SESSION), 1, LATER)
                .await?,
            CasOutcome::Conflict,
            "una revocación repetida no vuelve a actualizar"
        );
    }
    connection.close().await?;

    // Reintento de autenticación: la sesión revocada no construye principal.
    let mut connection = pool.acquire().await?;
    let result = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection))
        .authenticate(&hash, utc(1_793_235_600), Some(&id(BRANCH)))
        .await;
    assert!(result.is_err(), "una sesión revocada no puede autenticarse");
    Ok(())
}

#[tokio::test]
async fn second_bootstrap_attempt_is_rejected_and_leaves_no_partial_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")
        .unwrap_or_else(|error| panic!("capacidad: {error}"));

    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let mut transaction = unit_of_work.begin().await?;
    provision_initial_installation(&mut transaction, &capability, &request(), TIME).await?;
    transaction.commit().await?;

    // Segundo intento sobre la misma instalación: el UPDATE no afecta filas.
    let mut transaction = unit_of_work.begin().await?;
    let second = provision_initial_installation(
        &mut transaction,
        &capability,
        &BootstrapRequest {
            organization_id: id("00000000-0000-0000-0000-0000000000a2"),
            organization: NewOrganization {
                name: DisplayName::new("Otra").unwrap_or_else(|error| panic!("name: {error}")),
                code: BusinessCode::new("otra").unwrap_or_else(|error| panic!("code: {error}")),
                timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("tz: {error}")),
                currency: IsoCurrency::new("MXN")
                    .unwrap_or_else(|error| panic!("currency: {error}")),
                locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
            },
            ..request()
        },
        LATER,
    )
    .await;
    assert!(second.is_err(), "el bootstrap es one-shot por instalación");
    transaction.rollback().await?;

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization")
            .fetch_one(&pool)
            .await?,
        1,
        "el intento fallido no deja organizaciones parciales"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event")
            .fetch_one(&pool)
            .await?,
        1
    );
    Ok(())
}
