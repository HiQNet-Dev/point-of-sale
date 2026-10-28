use super::*;
use std::time::Duration;

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
