//! Credencial inicial, fallos por etapa y recuperación del provisioning candidato.
use super::*;
use crate::credential::{CredentialTransition, SqliteCredentialLab, prepare};
use pos_core::identity::{CredentialKind, LockoutPolicy};
use pos_credentials::{Argon2Policy, VerificationOutcome, VerifyRequest, verify_credential};

const PASSWORD: &str = "contraseña-sintetica-inicial-2d";

async fn assert_empty(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let total: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM organization) + (SELECT COUNT(*) FROM pos_user) \
         + (SELECT COUNT(*) FROM installation_identity) + (SELECT COUNT(*) FROM local_credential) \
         + (SELECT COUNT(*) FROM role) + (SELECT COUNT(*) FROM role_permission) \
         + (SELECT COUNT(*) FROM user_organization_role) + (SELECT COUNT(*) FROM bootstrap_record) \
         + (SELECT COUNT(*) FROM audit_event)",
    )
    .fetch_one(pool)
    .await?;
    assert_eq!(total, 0, "provisioning parcial confirmado");
    Ok(())
}

#[tokio::test]
async fn initial_password_verifies_after_reopen_without_recovering_plaintext()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let request = request();
    let debug = format!("{request:?}");
    assert!(!debug.contains(PASSWORD));
    assert!(
        !debug.contains(
            request
                .initial_credential
                .credential()
                .secret_hash()
                .expose_to_verifier()
        )
    );
    let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    provision_initial_installation(&mut transaction, &capability, &request, TIME).await?;
    transaction.commit().await?;
    pool.close().await;
    let pool = open(&temp.path().join("flow.sqlite"), Duration::from_secs(5)).await?;
    let mut connection = pool.acquire().await?;
    let credential = SqliteCredentialLab::canonical(&mut connection)
        .load(&id(ORG), &id(ADMIN), CredentialKind::Password)
        .await?
        .ok_or("credencial ausente")?;
    for (password, expected) in [
        (
            PASSWORD,
            VerificationOutcome::Verified {
                rehash_required: false,
            },
        ),
        ("incorrecta", VerificationOutcome::Rejected),
    ] {
        assert_eq!(
            verify_credential(&VerifyRequest {
                purpose: CredentialKind::Password,
                algorithm: credential.algorithm(),
                stored: credential.secret_hash(),
                password
            })?,
            expected
        );
    }
    assert!(
        SqliteCredentialLab::canonical(&mut connection)
            .load(
                &id("00000000-0000-0000-0000-00000000abcd"),
                &id(ADMIN),
                CredentialKind::Password
            )
            .await?
            .is_none()
    );
    drop(connection);
    let (state, actor, session): (String, String, Option<String>) = sqlx::query_as(
        "SELECT b.state,a.actor_type,a.session_id FROM bootstrap_record b JOIN audit_event a ON a.action = 'bootstrap.completed'",
    ).fetch_one(&pool).await?;
    assert_eq!(state, "COMPLETED");
    assert_eq!(actor, "SYSTEM");
    assert!(session.is_none());
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn each_provisioning_failure_rolls_back_and_can_retry()
-> Result<(), Box<dyn std::error::Error>> {
    for failure in [
        "CREATE TRIGGER fail_stage BEFORE INSERT ON local_credential BEGIN SELECT RAISE(ABORT,'test'); END;",
        "CREATE TRIGGER fail_stage BEFORE INSERT ON role BEGIN SELECT RAISE(ABORT,'test'); END;",
        "CREATE TRIGGER fail_stage BEFORE INSERT ON role_permission BEGIN SELECT RAISE(ABORT,'test'); END;",
        "CREATE TRIGGER fail_stage BEFORE INSERT ON audit_event BEGIN SELECT RAISE(ABORT,'test'); END;",
    ] {
        let (_temp, pool) = open_candidate().await?;
        let request = request();
        sqlx::raw_sql(failure).execute(&pool).await?;
        let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")?;
        let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
        assert!(
            provision_initial_installation(&mut transaction, &capability, &request, TIME)
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT state FROM bootstrap_record")
                .fetch_one(&mut *transaction.connection())
                .await?,
            "PENDING"
        );
        transaction.rollback().await?;
        assert_empty(&pool).await?;
        sqlx::raw_sql("DROP TRIGGER fail_stage")
            .execute(&pool)
            .await?;
        let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
        provision_initial_installation(&mut transaction, &capability, &request, TIME).await?;
        transaction.commit().await?;
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_credential")
                .fetch_one(&pool)
                .await?,
            1
        );
        pool.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn interruption_before_commit_leaves_no_administrator_or_credential()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let request = request();
    let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    provision_initial_installation(&mut transaction, &capability, &request, TIME).await?;
    drop(transaction);
    assert_empty(&pool).await?;
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn mismatched_credential_owner_is_rejected_before_writes()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let mut request = request();
    request.initial_credential = candidate_password(id("00000000-0000-0000-0000-00000000abcd"));
    let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    assert!(
        provision_initial_installation(&mut transaction, &capability, &request, TIME)
            .await
            .is_err()
    );
    transaction.rollback().await?;
    assert_empty(&pool).await?;
    pool.close().await;
    Ok(())
}

#[test]
fn password_proof_rejects_a_different_secret() {
    let credential = candidate_password(id(ADMIN)).credential().clone();
    assert!(
        pos_application::bootstrap::VerifiedInitialCredential::verify(
            &crate::bootstrap::CandidateBootstrapPasswords::lab_v1(),
            credential,
            "otra-contraseña",
        )
        .is_err()
    );
}

#[tokio::test]
async fn canonical_reset_uses_the_frozen_physical_state() -> Result<(), Box<dyn std::error::Error>>
{
    let (_temp, pool) = open_candidate().await?;
    let request = request();
    let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    provision_initial_installation(&mut transaction, &capability, &request, TIME).await?;
    transaction.commit().await?;
    sqlx::query("UPDATE local_credential SET secret_hash = 'corrupt-fixture'")
        .execute(&pool)
        .await?;
    let mut connection = pool.acquire().await?;
    let credential = SqliteCredentialLab::canonical(&mut connection)
        .load(&id(ORG), &id(ADMIN), CredentialKind::Password)
        .await?
        .ok_or("sin credencial")?;
    drop(connection);
    let prepared = prepare(
        &credential,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    let applied = SqliteCredentialLab::canonical(transaction.connection())
        .apply(
            &id(ORG),
            &id(ADMIN),
            CredentialKind::Password,
            prepared,
            utc(1_793_235_600),
            LockoutPolicy::new(1, 60)?,
        )
        .await?;
    assert_eq!(applied.transition, CredentialTransition::ResetRequired);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM local_credential")
            .fetch_one(&mut *transaction.connection())
            .await?,
        "PASSWORD_RESET_REQUIRED"
    );
    transaction.rollback().await?; // Ensayo de mapper; no confirma mutación sin auditoría.
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn disabled_user_cannot_apply_a_prepared_password_result()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, pool) = open_candidate().await?;
    let request = request();
    let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    provision_initial_installation(&mut transaction, &capability, &request, TIME).await?;
    transaction.commit().await?;
    let credential = request.initial_credential.credential();
    let prepared = prepare(
        credential,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;
    sqlx::query("UPDATE pos_user SET status = 'INACTIVE'")
        .execute(&pool)
        .await?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    let applied = SqliteCredentialLab::canonical(transaction.connection())
        .apply(
            &id(ORG),
            &id(ADMIN),
            CredentialKind::Password,
            prepared,
            utc(1_793_235_600),
            LockoutPolicy::new(1, 60)?,
        )
        .await?;
    assert_eq!(applied.transition, CredentialTransition::Rejected);
    assert!(!applied.persisted);
    transaction.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT credential_version FROM local_credential")
            .fetch_one(&pool)
            .await?,
        1
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn account_lock_survives_reopen_and_resets_after_cooldown()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, pool) = open_candidate().await?;
    let request = request();
    let capability = authorize_bootstrap(&TestVerifier, b"secreto-de-prueba")?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    provision_initial_installation(&mut transaction, &capability, &request, TIME).await?;
    // Reutiliza el administrador existente y su fixture de sesión, no otro actor.
    sqlx::query("INSERT INTO local_session (id,user_id,session_hash,created_at,expires_at) VALUES (?,?,?,?,?)")
        .bind(SESSION).bind(ADMIN).bind(HASH).bind(TIME).bind(LATER).execute(&mut *transaction.connection()).await?;
    transaction.commit().await?;
    let now = utc(1_793_235_600);
    let mut connection = pool.acquire().await?;
    let credential = SqliteCredentialLab::canonical(&mut connection)
        .load(&id(ORG), &id(ADMIN), CredentialKind::Password)
        .await?
        .ok_or("sin credencial")?;
    let principal = LocalPrincipalService::new(SqliteIdentityLab::canonical(&mut connection))
        .authenticate(&SessionHash::new(HASH)?, now, None)
        .await?;
    drop(connection);
    let prepared = prepare(
        &credential,
        CredentialKind::Password,
        "incorrecta",
        Argon2Policy::LAB_V1,
    )?;
    let lockout = LockoutPolicy::new(1, 60)?; // Política de prueba inyectada, no comercial.
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    let applied = SqliteCredentialLab::canonical(transaction.connection())
        .apply(
            &id(ORG),
            &id(ADMIN),
            CredentialKind::Password,
            prepared,
            now,
            lockout,
        )
        .await?;
    assert_eq!(applied.transition, CredentialTransition::Locked);
    let event = pos_application::audit::build_event(
        id(BRANCH_EVENT),
        &principal,
        "credential.verification_failed",
        "credential",
        credential.id().clone(),
        serde_json::json!({}),
        &AuditOccurredAt::new("2026-10-29T01:00:00Z")?,
    )?;
    SqliteAuditWriter::append(&mut transaction, &event).await?;
    transaction.commit().await?;
    pool.close().await;
    let pool = open(&temp.path().join("flow.sqlite"), Duration::from_secs(5)).await?;
    let mut connection = pool.acquire().await?;
    let locked = SqliteCredentialLab::canonical(&mut connection)
        .load(&id(ORG), &id(ADMIN), CredentialKind::Password)
        .await?
        .ok_or("sin credencial")?;
    assert_eq!(locked.failed_login_attempts(), 1);
    assert!(locked.is_locked(utc(now.get() + 59)));
    assert!(locked.accepts_verification(utc(now.get() + 59)).is_err());
    drop(connection);
    let prepared = prepare(
        &locked,
        CredentialKind::Password,
        PASSWORD,
        Argon2Policy::LAB_V1,
    )?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    let applied = SqliteCredentialLab::canonical(transaction.connection())
        .apply(
            &id(ORG),
            &id(ADMIN),
            CredentialKind::Password,
            prepared,
            utc(now.get() + 60),
            lockout,
        )
        .await?;
    assert_eq!(
        applied.transition,
        CredentialTransition::Verified {
            rehash_performed: false
        }
    );
    let event = pos_application::audit::build_event(
        id("00000000-0000-0000-0000-000000002d02"),
        &principal,
        "credential.verified",
        "credential",
        credential.id().clone(),
        serde_json::json!({}),
        &AuditOccurredAt::new("2026-10-29T01:01:00Z")?,
    )?;
    SqliteAuditWriter::append(&mut transaction, &event).await?;
    transaction.commit().await?;
    let mut connection = pool.acquire().await?;
    let cleared = SqliteCredentialLab::canonical(&mut connection)
        .load(&id(ORG), &id(ADMIN), CredentialKind::Password)
        .await?
        .ok_or("sin credencial")?;
    assert_eq!(cleared.failed_login_attempts(), 0);
    assert!(cleared.locked_until().is_none());
    drop(connection);
    pool.close().await;
    Ok(())
}
