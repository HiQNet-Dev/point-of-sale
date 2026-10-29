use super::*;
use pos_core::organization_branch::{
    Branch, BranchPatch, BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewBranch,
    NewOrganization, Organization, Timezone, TimezoneOverridePatch,
};
use std::time::Duration;

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
