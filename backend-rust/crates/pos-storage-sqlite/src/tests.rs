// Los fixtures de estos tests son constantes válidas; los panics solo señalan un
// error de programación del test y no forman parte del código de producto.
#![allow(clippy::panic)]

use super::*;
use pos_core::{
    audit::AuditMetadata,
    organization_branch::{
        ApplicationError, AuditEvent, Authorization, Branch, BranchPatch, BusinessCode,
        DisplayName, EntityId, IdentityGenerator, IsoCurrency, LocaleTag, NewBranch,
        NewOrganization, Organization, OrganizationPatch, Permission, Timezone,
        TimezoneOverridePatch,
    },
};
use serde_json::json;
use std::{collections::VecDeque, future::pending, time::Duration};
use tokio::{sync::oneshot, time::timeout};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn organization(id_value: &str, code: &str) -> Organization {
    Organization::create(
        id(id_value),
        NewOrganization {
            name: DisplayName::new("Laboratorio").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone: Timezone::new("UTC").unwrap_or_else(|error| panic!("timezone: {error}")),
            currency: IsoCurrency::new("MXN").unwrap_or_else(|error| panic!("currency: {error}")),
            locale: LocaleTag::new("es-MX").unwrap_or_else(|error| panic!("locale: {error}")),
        },
    )
}

fn branch(id_value: &str, organization_id: EntityId, code: &str) -> Branch {
    Branch::create(
        id(id_value),
        organization_id,
        NewBranch {
            name: DisplayName::new("Sucursal").unwrap_or_else(|error| panic!("name: {error}")),
            code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
            timezone_override: None,
        },
    )
}

const TIME: &str = "2026-10-29T00:00:00Z";

#[tokio::test]
async fn validates_file_and_timeout_before_opening() -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        open(std::path::Path::new(":memory:"), Duration::from_millis(50))
            .await
            .is_err()
    );
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("local.sqlite");
    assert!(open(&path, Duration::ZERO).await.is_err());
    assert!(!path.exists());
    Ok(())
}

#[tokio::test]
async fn configures_and_verifies_every_new_connection() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(&temp.path().join("local.sqlite"), Duration::from_millis(75)).await?;
    let mut connection = pool.acquire().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&mut *connection)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
            .fetch_one(&mut *connection)
            .await?,
        "wal"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA busy_timeout")
            .fetch_one(&mut *connection)
            .await?,
        75
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA synchronous")
            .fetch_one(&mut *connection)
            .await?,
        2
    );
    connection.close().await?;
    let mut replacement = pool.acquire().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&mut *replacement)
            .await?,
        1
    );
    drop(replacement);
    checkpoint(&pool).await?;
    pool.close().await;
    assert!(pool.is_closed());
    Ok(())
}

#[tokio::test]
async fn clean_subset_is_strict_and_migration_replay_is_noop()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(&temp.path().join("local.sqlite"), Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    migrate(&pool).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await?,
        1
    );
    let strict = sqlx::query_scalar::<_, i64>(
        "SELECT strict FROM pragma_table_list WHERE name='organization'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(strict, 1);
    assert!(sqlx::query("INSERT INTO branch (id, organization_id, code, name, status, created_at, updated_at) VALUES ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','LAB','Lab','ACTIVE','2026-10-27T00:00:00Z','2026-10-27T00:00:00Z')").execute(&pool).await.is_err());
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn rejects_noncanonical_ids_and_currency_including_nul_suffixes()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("checks.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    let insert_org = "INSERT INTO organization (id, code, name, currency, timezone, locale, stock_negative_policy, created_at, updated_at) VALUES (?, ?, 'Lab', ?, 'UTC', 'es-MX', 'STRICT', '2026-10-27T00:00:00Z', '2026-10-27T00:00:00Z')";
    let id = "00000000-0000-0000-0000-000000000001";
    for (candidate, currency) in [
        (format!("{id}\0invalid"), "MXN"),
        (id.into(), "MXN\0invalid"),
        ("not-a-uuid".into(), "MXN"),
        (id.into(), "mxn"),
    ] {
        let rejected = sqlx::query(insert_org)
            .bind(candidate)
            .bind("INVALID")
            .bind(currency)
            .execute(&pool)
            .await;
        assert!(
            rejected
                .as_ref()
                .err()
                .and_then(sqlx::Error::as_database_error)
                .is_some_and(|error| error.is_check_violation()),
            "debe rechazarse por CHECK"
        );
    }
    sqlx::query(insert_org)
        .bind(id)
        .bind("VALID")
        .bind("MXN")
        .execute(&pool)
        .await?;
    let branch_insert = "INSERT INTO branch (id, organization_id, code, name, created_at, updated_at) VALUES (?, ?, 'BRANCH', 'Lab', '2026-10-27T00:00:00Z', '2026-10-27T00:00:00Z')";
    let rejected = sqlx::query(branch_insert)
        .bind(format!("{id}\0invalid"))
        .bind(id)
        .execute(&pool)
        .await;
    assert!(
        rejected
            .as_ref()
            .err()
            .and_then(sqlx::Error::as_database_error)
            .is_some_and(|error| error.is_check_violation())
    );
    sqlx::query(branch_insert)
        .bind("00000000-0000-0000-0000-000000000002")
        .bind(id)
        .execute(&pool)
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization")
            .fetch_one(&pool)
            .await?,
        1
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn organization_branch_store_persists_scopes_constraints_and_cas()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("organization-branch.sqlite");
    let pool = open(&path, Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    let first = organization("00000000-0000-0000-0000-000000000010", "main");
    let second = organization("00000000-0000-0000-0000-000000000020", "other");
    store.insert_organization(&first, TIME, TIME).await?;
    store.insert_organization(&second, TIME, TIME).await?;
    assert_eq!(
        store
            .insert_organization(
                &organization("00000000-0000-0000-0000-000000000030", " MAIN "),
                TIME,
                TIME
            )
            .await,
        Err(OrganizationBranchStorageError::DuplicateOrganizationCode)
    );
    let first_branch = branch(
        "00000000-0000-0000-0000-000000000011",
        first.id().clone(),
        "center",
    );
    store.insert_branch(&first_branch, TIME, TIME).await?;
    assert_eq!(
        store
            .insert_branch(
                &branch(
                    "00000000-0000-0000-0000-000000000012",
                    id("00000000-0000-0000-0000-000000000099"),
                    "orphan"
                ),
                TIME,
                TIME
            )
            .await,
        Err(OrganizationBranchStorageError::ForeignKey)
    );
    assert_eq!(
        store.get_branch(second.id(), first_branch.id()).await?,
        None
    );
    assert_eq!(store.list_branches(first.id()).await?.len(), 1);
    assert!(store.list_branches(second.id()).await?.is_empty());

    let mut changed = store
        .get_branch(first.id(), first_branch.id())
        .await?
        .ok_or_else(|| std::io::Error::other("missing fixture branch"))?;
    assert!(changed.patch(
        first.id(),
        BranchPatch {
            expected_revision: 0,
            name: Some(DisplayName::new("Sucursal Norte")?),
            timezone_override: TimezoneOverridePatch::Keep,
            status: None
        }
    )?);
    assert_eq!(
        store
            .update_branch_cas(first.id(), 0, &changed, TIME)
            .await?,
        CasOutcome::Updated
    );
    assert_eq!(
        store
            .update_branch_cas(first.id(), 0, &changed, TIME)
            .await?,
        CasOutcome::Conflict
    );
    assert_eq!(
        store
            .update_branch_cas(second.id(), 1, &changed, TIME)
            .await?,
        CasOutcome::Missing
    );
    assert_eq!(
        store
            .get_branch(first.id(), first_branch.id())
            .await?
            .ok_or_else(|| std::io::Error::other("missing persisted branch"))?
            .revision(),
        1
    );
    pool.close().await;

    let reopened = open(&path, Duration::from_millis(75)).await?;
    let reloaded = SqliteOrganizationBranchStore::new(reopened.clone())
        .get_organization(first.id())
        .await?;
    assert_eq!(
        reloaded
            .ok_or_else(|| std::io::Error::other("missing durable organization"))?
            .code()
            .as_str(),
        "main"
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn dropped_transaction_rolls_back_before_the_connection_is_reused()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(&temp.path().join("drop.sqlite"), Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let abandoned = organization("00000000-0000-0000-0000-000000000041", "abandoned");

    let mut transaction = unit_of_work.begin().await?;
    transaction.insert_organization(&abandoned, TIME).await?;
    drop(transaction);

    // SQLx encola rollback en Drop; la siguiente operación de la conexión lo
    // procesa antes de reutilizarla. El timeout evita que el test oculte un
    // lock que haya quedado abierto.
    timeout(Duration::from_secs(1), unit_of_work.begin())
        .await??
        .rollback()
        .await?;
    assert!(
        SqliteOrganizationBranchStore::new(pool.clone())
            .get_organization(abandoned.id())
            .await?
            .is_none()
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn aborted_task_rolls_back_and_the_pool_recovers() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(&temp.path().join("abort.sqlite"), Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let written = organization("00000000-0000-0000-0000-000000000042", "cancelled");
    let (ready, received) = oneshot::channel();
    let worker_uow = unit_of_work.clone();
    let worker_organization = written.clone();
    let worker = tokio::spawn(async move {
        let mut transaction = worker_uow.begin().await?;
        transaction
            .insert_organization(&worker_organization, TIME)
            .await?;
        let _ = ready.send(());
        pending::<()>().await;
        #[allow(unreachable_code)]
        Ok::<(), OrganizationBranchStorageError>(())
    });
    received.await?;
    worker.abort();
    assert!(worker.await.is_err_and(|error| error.is_cancelled()));

    timeout(Duration::from_secs(1), unit_of_work.begin())
        .await??
        .rollback()
        .await?;
    assert!(
        SqliteOrganizationBranchStore::new(pool.clone())
            .get_organization(written.id())
            .await?
            .is_none()
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn begin_immediate_blocks_an_independent_writer_but_not_a_wal_reader()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("immediate.sqlite");
    let first_pool = open(&path, Duration::from_millis(75)).await?;
    migrate(&first_pool).await?;
    let second_pool = open(&path, Duration::from_millis(75)).await?;
    let first = SqliteUnitOfWork::new(first_pool.clone());
    let second = SqliteUnitOfWork::new(second_pool.clone());

    let transaction = first.begin().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization")
            .fetch_one(&second_pool)
            .await?,
        0
    );
    assert!(matches!(
        second.begin().await,
        Err(OrganizationBranchStorageError::Database)
    ));
    drop(transaction);

    timeout(Duration::from_secs(1), second.begin())
        .await??
        .rollback()
        .await?;
    first_pool.close().await;
    second_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn explicit_rollback_and_failed_commit_never_report_a_business_success()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("commit.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    sqlx::raw_sql(
        "CREATE TABLE commit_parent (id INTEGER PRIMARY KEY) STRICT;
         CREATE TABLE commit_child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED) STRICT;",
    )
    .execute(&pool)
    .await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());

    let rolled_back = organization("00000000-0000-0000-0000-000000000043", "explicit");
    let mut transaction = unit_of_work.begin().await?;
    transaction.insert_organization(&rolled_back, TIME).await?;
    transaction.rollback().await?;
    assert!(
        SqliteOrganizationBranchStore::new(pool.clone())
            .get_organization(rolled_back.id())
            .await?
            .is_none()
    );

    let mut transaction = unit_of_work.begin().await?;
    sqlx::query("INSERT INTO commit_child (id, parent_id) VALUES (1, 999)")
        .execute(transaction.connection())
        .await?;
    assert_eq!(
        transaction.commit().await,
        Err(OrganizationBranchStorageError::Database)
    );
    timeout(Duration::from_secs(1), unit_of_work.begin())
        .await??
        .rollback()
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM commit_child")
            .fetch_one(&pool)
            .await?,
        0
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn concurrent_cas_allows_one_organization_and_branch_writer_per_revision()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("cas-concurrent.sqlite");
    let first_pool = open(&path, Duration::from_millis(500)).await?;
    migrate(&first_pool).await?;
    let second_pool = open(&path, Duration::from_millis(500)).await?;
    let first_store = SqliteOrganizationBranchStore::new(first_pool.clone());
    let second_store = SqliteOrganizationBranchStore::new(second_pool.clone());
    let organization = organization("00000000-0000-0000-0000-000000000044", "cas-org");
    first_store
        .insert_organization(&organization, TIME, TIME)
        .await?;
    let branch = branch(
        "00000000-0000-0000-0000-000000000045",
        organization.id().clone(),
        "cas-branch",
    );
    first_store.insert_branch(&branch, TIME, TIME).await?;

    let mut first_organization = first_store
        .get_organization(organization.id())
        .await?
        .ok_or_else(|| std::io::Error::other("missing organization"))?;
    let mut second_organization = second_store
        .get_organization(organization.id())
        .await?
        .ok_or_else(|| std::io::Error::other("missing organization"))?;
    assert!(first_organization.patch(OrganizationPatch {
        expected_revision: 0,
        name: Some(DisplayName::new("Primera organización")?),
        timezone: None,
        currency: None,
        locale: None,
        status: None,
        negative_stock_policy: None,
    })?);
    assert!(second_organization.patch(OrganizationPatch {
        expected_revision: 0,
        name: Some(DisplayName::new("Segunda organización")?),
        timezone: None,
        currency: None,
        locale: None,
        status: None,
        negative_stock_policy: None,
    })?);
    let (first_result, second_result) = tokio::join!(
        first_store.update_organization_cas(0, &first_organization, TIME),
        second_store.update_organization_cas(0, &second_organization, TIME)
    );
    let organization_outcomes = [first_result?, second_result?];
    assert_eq!(
        organization_outcomes
            .into_iter()
            .filter(|outcome| *outcome == CasOutcome::Updated)
            .count(),
        1
    );
    assert_eq!(
        organization_outcomes
            .into_iter()
            .filter(|outcome| *outcome == CasOutcome::Conflict)
            .count(),
        1
    );

    let mut first_branch = first_store
        .get_branch(organization.id(), branch.id())
        .await?
        .ok_or_else(|| std::io::Error::other("missing branch"))?;
    let mut second_branch = second_store
        .get_branch(organization.id(), branch.id())
        .await?
        .ok_or_else(|| std::io::Error::other("missing branch"))?;
    assert!(first_branch.patch(
        organization.id(),
        BranchPatch {
            expected_revision: 0,
            name: Some(DisplayName::new("Primera sucursal")?),
            timezone_override: TimezoneOverridePatch::Keep,
            status: None,
        },
    )?);
    assert!(second_branch.patch(
        organization.id(),
        BranchPatch {
            expected_revision: 0,
            name: Some(DisplayName::new("Segunda sucursal")?),
            timezone_override: TimezoneOverridePatch::Keep,
            status: None,
        },
    )?);
    let (first_result, second_result) = tokio::join!(
        first_store.update_branch_cas(organization.id(), 0, &first_branch, TIME),
        second_store.update_branch_cas(organization.id(), 0, &second_branch, TIME)
    );
    let branch_outcomes = [first_result?, second_result?];
    assert_eq!(
        branch_outcomes
            .into_iter()
            .filter(|outcome| *outcome == CasOutcome::Updated)
            .count(),
        1
    );
    assert_eq!(
        branch_outcomes
            .into_iter()
            .filter(|outcome| *outcome == CasOutcome::Conflict)
            .count(),
        1
    );
    first_pool.close().await;
    second_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn audit_event_fixture_enforces_append_only_actor_scope_metadata_and_atomicity()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("audit-event.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    let store = SqliteOrganizationBranchStore::new(pool.clone());
    let organization_a = organization("00000000-0000-0000-0000-000000000070", "audit-a");
    let organization_b = organization("00000000-0000-0000-0000-000000000071", "audit-b");
    let branch_b = branch(
        "00000000-0000-0000-0000-000000000072",
        organization_b.id().clone(),
        "audit-b-branch",
    );
    store
        .insert_organization(&organization_a, TIME, TIME)
        .await?;
    store
        .insert_organization(&organization_b, TIME, TIME)
        .await?;
    store.insert_branch(&branch_b, TIME, TIME).await?;
    sqlx::query(
        "CREATE TABLE audit_event_fixture (
            id TEXT PRIMARY KEY NOT NULL,
            organization_id TEXT NOT NULL REFERENCES organization(id),
            branch_id TEXT,
            actor_type TEXT NOT NULL CHECK (actor_type IN ('USER', 'SYSTEM')),
            actor_user_id TEXT,
            action TEXT NOT NULL CHECK (length(trim(action)) > 0),
            entity_type TEXT NOT NULL CHECK (length(trim(entity_type)) > 0),
            metadata TEXT CHECK (metadata IS NULL OR (json_valid(metadata) AND length(CAST(metadata AS BLOB)) <= 16384)),
            occurred_at TEXT NOT NULL,
            CHECK ((actor_type = 'USER' AND actor_user_id IS NOT NULL) OR (actor_type = 'SYSTEM' AND actor_user_id IS NULL)),
            FOREIGN KEY (branch_id, organization_id) REFERENCES branch(id, organization_id)
        ) STRICT",
    )
    .execute(&pool)
    .await?;
    sqlx::query("CREATE TRIGGER audit_event_fixture_reject_update BEFORE UPDATE ON audit_event_fixture BEGIN SELECT RAISE(ABORT, 'audit fixture is append-only'); END")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE TRIGGER audit_event_fixture_reject_delete BEFORE DELETE ON audit_event_fixture BEGIN SELECT RAISE(ABORT, 'audit fixture is append-only'); END")
        .execute(&pool)
        .await?;

    let metadata = AuditMetadata::new(json!({ "changedFields": ["name"] }))?;
    let mut transaction = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    sqlx::query("UPDATE organization SET name = 'Audited change' WHERE id = ?")
        .bind(organization_a.id().as_str())
        .execute(transaction.connection())
        .await?;
    sqlx::query("INSERT INTO audit_event_fixture (id, organization_id, branch_id, actor_type, actor_user_id, action, entity_type, metadata, occurred_at) VALUES (?, ?, NULL, 'SYSTEM', NULL, 'organization.updated', 'organization', ?, ?)")
        .bind("00000000-0000-0000-0000-000000000073")
        .bind(organization_a.id().as_str())
        .bind(metadata.canonical_json())
        .bind(TIME)
        .execute(transaction.connection())
        .await?;
    transaction.commit().await?;

    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM organization WHERE id = ?")
            .bind(organization_a.id().as_str())
            .fetch_one(&pool)
            .await?,
        "Audited change"
    );
    assert!(
        sqlx::query("UPDATE audit_event_fixture SET action = 'hacked'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM audit_event_fixture")
            .execute(&pool)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&pool)
            .await?,
        1
    );
    assert!(sqlx::query("INSERT INTO audit_event_fixture (id, organization_id, actor_type, actor_user_id, action, entity_type, occurred_at) VALUES (?, ?, 'USER', NULL, 'x.updated', 'x', ?)")
        .bind("00000000-0000-0000-0000-000000000074")
        .bind(organization_a.id().as_str())
        .bind(TIME)
        .execute(&pool)
        .await
        .is_err());
    assert!(sqlx::query("INSERT INTO audit_event_fixture (id, organization_id, branch_id, actor_type, action, entity_type, occurred_at) VALUES (?, ?, ?, 'SYSTEM', 'x.updated', 'x', ?)")
        .bind("00000000-0000-0000-0000-000000000075")
        .bind(organization_a.id().as_str())
        .bind(branch_b.id().as_str())
        .bind(TIME)
        .execute(&pool)
        .await
        .is_err());
    assert!(sqlx::query("INSERT INTO audit_event_fixture (id, organization_id, actor_type, action, entity_type, metadata, occurred_at) VALUES (?, ?, 'SYSTEM', 'x.updated', 'x', 'not-json', ?)")
        .bind("00000000-0000-0000-0000-000000000076")
        .bind(organization_a.id().as_str())
        .bind(TIME)
        .execute(&pool)
        .await
        .is_err());

    let mut rollback = SqliteUnitOfWork::new(pool.clone()).begin().await?;
    sqlx::query("UPDATE organization SET name = 'Must rollback' WHERE id = ?")
        .bind(organization_a.id().as_str())
        .execute(rollback.connection())
        .await?;
    assert!(sqlx::query("INSERT INTO audit_event_fixture (id, organization_id, actor_type, actor_user_id, action, entity_type, occurred_at) VALUES (?, ?, 'USER', NULL, 'x.updated', 'x', ?)")
        .bind("00000000-0000-0000-0000-000000000077")
        .bind(organization_a.id().as_str())
        .bind(TIME)
        .execute(rollback.connection())
        .await
        .is_err());
    rollback.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM organization WHERE id = ?")
            .bind(organization_a.id().as_str())
            .fetch_one(&pool)
            .await?,
        "Audited change"
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn audit_fixture_event_is_rolled_back_when_its_task_is_cancelled()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("audit-cancel.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    let organization = organization("00000000-0000-0000-0000-000000000078", "audit-cancel");
    SqliteOrganizationBranchStore::new(pool.clone())
        .insert_organization(&organization, TIME, TIME)
        .await?;
    sqlx::query("CREATE TABLE audit_event_fixture (id TEXT PRIMARY KEY, organization_id TEXT NOT NULL REFERENCES organization(id), action TEXT NOT NULL, occurred_at TEXT NOT NULL) STRICT")
        .execute(&pool)
        .await?;
    let unit_of_work = SqliteUnitOfWork::new(pool.clone());
    let (ready, received) = oneshot::channel();
    let organization_id = organization.id().clone();
    let worker = tokio::spawn(async move {
        let mut transaction = unit_of_work.begin().await?;
        sqlx::query("UPDATE organization SET name = 'Cancelled audit' WHERE id = ?")
            .bind(organization_id.as_str())
            .execute(transaction.connection())
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        sqlx::query("INSERT INTO audit_event_fixture (id, organization_id, action, occurred_at) VALUES (?, ?, 'organization.updated', ?)")
            .bind("00000000-0000-0000-0000-000000000079")
            .bind(organization_id.as_str())
            .bind(TIME)
            .execute(transaction.connection())
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        let _ = ready.send(());
        pending::<()>().await;
        #[allow(unreachable_code)]
        Ok::<(), OrganizationBranchStorageError>(())
    });
    received.await?;
    worker.abort();
    assert!(worker.await.is_err_and(|error| error.is_cancelled()));
    timeout(
        Duration::from_secs(1),
        SqliteUnitOfWork::new(pool.clone()).begin(),
    )
    .await??
    .rollback()
    .await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM organization WHERE id = ?")
            .bind(organization.id().as_str())
            .fetch_one(&pool)
            .await?,
        "Laboratorio"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_event_fixture")
            .fetch_one(&pool)
            .await?,
        0
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn real_sqlite_transaction_rolls_back_business_write_when_audit_fixture_fails()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("rollback.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    sqlx::query("CREATE TABLE audit_fixture (action TEXT NOT NULL CHECK(action <> 'forced-failure')) STRICT")
        .execute(&pool)
        .await?;
    let mut transaction = pool.begin().await?;
    sqlx::query("INSERT INTO organization (id, code, name, currency, timezone, locale, stock_negative_policy, status, version, created_at, updated_at) VALUES ('00000000-0000-0000-0000-000000000010', 'rollback', 'Rollback', 'MXN', 'UTC', 'es-MX', 'STRICT', 'ACTIVE', 0, ?, ?)")
        .bind(TIME)
        .bind(TIME)
        .execute(&mut *transaction)
        .await?;
    assert!(
        sqlx::query("INSERT INTO audit_fixture (action) VALUES ('forced-failure')")
            .execute(&mut *transaction)
            .await
            .is_err()
    );
    transaction.rollback().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization WHERE code = 'rollback'")
            .fetch_one(&pool)
            .await?,
        0
    );
    pool.close().await;
    Ok(())
}

struct FixtureAuthorization(bool);
impl Authorization for FixtureAuthorization {
    fn is_allowed(&self, _: Permission, _: Option<&EntityId>, _: Option<&EntityId>) -> bool {
        self.0
    }
}

struct FixtureIds(VecDeque<EntityId>);
impl IdentityGenerator for FixtureIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> {
        self.0.pop_front().ok_or(ApplicationError::Storage)
    }
}

struct FixtureAudit {
    fail: bool,
}

#[allow(clippy::manual_async_fn)]
impl TransactionalAuditWriter for FixtureAudit {
    async fn append(
        &self,
        transaction: &mut SqliteTransaction,
        event: &AuditEvent,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        if self.fail {
            return Err(ApplicationError::Audit);
        }
        sqlx::query("INSERT INTO audit_fixture (organization_id, branch_id, action, entity_type, entity_id, occurred_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(event.organization_id.as_str())
            .bind(event.branch_id.as_ref().map(EntityId::as_str))
            .bind(event.action)
            .bind(event.entity_type)
            .bind(event.entity_id.as_str())
            .bind(occurred_at)
            .execute(transaction.connection())
            .await
            .map_err(|_| ApplicationError::Audit)?;
        Ok(())
    }
}

#[tokio::test]
async fn async_organization_bootstrap_get_update_and_audit_are_atomic()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("organization-uow.sqlite"),
        Duration::from_millis(75),
    )
    .await?;
    migrate(&pool).await?;
    sqlx::query("CREATE TABLE audit_fixture (organization_id TEXT NOT NULL, branch_id TEXT, action TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id TEXT NOT NULL, occurred_at TEXT NOT NULL) STRICT")
        .execute(&pool)
        .await?;
    let organization_id = id("00000000-0000-0000-0000-000000000051");
    let mut ids = FixtureIds(VecDeque::from([organization_id.clone()]));
    let application = SqliteOrganizationApplication::new(
        SqliteUnitOfWork::new(pool.clone()),
        FixtureAudit { fail: false },
    );
    let created = application
        .bootstrap_create(
            &FixtureAuthorization(true),
            &mut ids,
            NewOrganization {
                name: DisplayName::new("Principal")?,
                code: BusinessCode::new("main")?,
                timezone: Timezone::new("UTC")?,
                currency: IsoCurrency::new("MXN")?,
                locale: LocaleTag::new("es-MX")?,
            },
            TIME,
        )
        .await?;
    assert_eq!(created.id(), &organization_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_fixture WHERE action = 'organization.created'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    assert_eq!(
        application
            .get(&FixtureAuthorization(true), &organization_id)
            .await?,
        created
    );

    let updated = application
        .update(
            &FixtureAuthorization(true),
            &organization_id,
            OrganizationPatch {
                expected_revision: 0,
                name: Some(DisplayName::new("Principal Norte")?),
                timezone: None,
                currency: None,
                locale: None,
                status: None,
                negative_stock_policy: None,
            },
            TIME,
        )
        .await?;
    assert_eq!(updated.revision(), 1);
    let no_op = application
        .update(
            &FixtureAuthorization(true),
            &organization_id,
            OrganizationPatch {
                expected_revision: 1,
                name: None,
                timezone: None,
                currency: None,
                locale: None,
                status: None,
                negative_stock_policy: None,
            },
            TIME,
        )
        .await?;
    assert_eq!(no_op.revision(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_fixture WHERE action = 'organization.updated'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );

    let failing = SqliteOrganizationApplication::new(
        SqliteUnitOfWork::new(pool.clone()),
        FixtureAudit { fail: true },
    );
    assert_eq!(
        failing
            .update(
                &FixtureAuthorization(true),
                &organization_id,
                OrganizationPatch {
                    expected_revision: 1,
                    name: Some(DisplayName::new("No confirma")?),
                    timezone: None,
                    currency: None,
                    locale: None,
                    status: None,
                    negative_stock_policy: None,
                },
                TIME,
            )
            .await,
        Err(ApplicationError::Audit)
    );
    let persisted = SqliteOrganizationBranchStore::new(pool.clone())
        .get_organization(&organization_id)
        .await?
        .ok_or_else(|| std::io::Error::other("missing organization"))?;
    assert_eq!(persisted.name().as_str(), "Principal Norte");
    assert_eq!(persisted.revision(), 1);
    assert_eq!(
        application
            .get(&FixtureAuthorization(false), &organization_id)
            .await,
        Err(ApplicationError::Forbidden)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn async_uow_commits_cas_and_rolls_back_audit_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(&temp.path().join("uow.sqlite"), Duration::from_millis(75)).await?;
    migrate(&pool).await?;
    sqlx::query("CREATE TABLE audit_fixture (organization_id TEXT NOT NULL, branch_id TEXT, action TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id TEXT NOT NULL, occurred_at TEXT NOT NULL) STRICT")
        .execute(&pool)
        .await?;
    let organization = organization("00000000-0000-0000-0000-000000000010", "main");
    SqliteOrganizationBranchStore::new(pool.clone())
        .insert_organization(&organization, TIME, TIME)
        .await?;
    let mut ids = FixtureIds(VecDeque::from([id("00000000-0000-0000-0000-000000000011")]));
    let application = SqliteBranchApplication::new(
        SqliteUnitOfWork::new(pool.clone()),
        FixtureAudit { fail: false },
    );
    let branch = application
        .create(
            &FixtureAuthorization(true),
            &mut ids,
            organization.id(),
            NewBranch {
                name: DisplayName::new("Sucursal")?,
                code: BusinessCode::new("center")?,
                timezone_override: None,
            },
            TIME,
        )
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_fixture WHERE action = 'branch.created'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    let updated = application
        .update(
            &FixtureAuthorization(true),
            organization.id(),
            branch.id(),
            BranchPatch {
                expected_revision: 0,
                name: Some(DisplayName::new("Centro Norte")?),
                timezone_override: TimezoneOverridePatch::Keep,
                status: None,
            },
            TIME,
        )
        .await?;
    assert_eq!(updated.revision(), 1);
    let no_op = application
        .update(
            &FixtureAuthorization(true),
            organization.id(),
            branch.id(),
            BranchPatch {
                expected_revision: 1,
                name: None,
                timezone_override: TimezoneOverridePatch::Keep,
                status: None,
            },
            TIME,
        )
        .await?;
    assert_eq!(no_op.revision(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_fixture WHERE action = 'branch.updated'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    let failing = SqliteBranchApplication::new(
        SqliteUnitOfWork::new(pool.clone()),
        FixtureAudit { fail: true },
    );
    assert_eq!(
        failing
            .update(
                &FixtureAuthorization(true),
                organization.id(),
                branch.id(),
                BranchPatch {
                    expected_revision: 1,
                    name: Some(DisplayName::new("No confirma")?),
                    timezone_override: TimezoneOverridePatch::Keep,
                    status: None
                },
                TIME
            )
            .await,
        Err(ApplicationError::Audit)
    );
    let persisted = SqliteOrganizationBranchStore::new(pool.clone())
        .get_branch(organization.id(), branch.id())
        .await?
        .ok_or_else(|| std::io::Error::other("missing branch"))?;
    assert_eq!(persisted.name().as_str(), "Centro Norte");
    assert_eq!(persisted.revision(), 1);
    assert_eq!(
        failing
            .update(
                &FixtureAuthorization(false),
                organization.id(),
                branch.id(),
                BranchPatch {
                    expected_revision: 1,
                    name: Some(DisplayName::new("Denied")?),
                    timezone_override: TimezoneOverridePatch::Keep,
                    status: None
                },
                TIME
            )
            .await,
        Err(ApplicationError::Forbidden)
    );
    pool.close().await;
    Ok(())
}
