//! SQLite real: WAL, persistencia del snapshot y rechazo de sobrescritura.
use super::*;
use pos_core::organization_branch::{
    BusinessCode, DisplayName, EntityId, IsoCurrency, LocaleTag, NewOrganization, Organization,
    Timezone,
};
use std::time::Duration;

async fn insert_organization(
    pool: &SqlitePool,
    suffix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let organization = Organization::create(
        EntityId::parse(&format!("00000000-0000-0000-0000-{suffix:0>12}"))?,
        NewOrganization {
            code: BusinessCode::new(&format!("org-{suffix}"))?,
            name: DisplayName::new("Respaldo sintético")?,
            currency: IsoCurrency::new("MXN")?,
            timezone: Timezone::new("UTC")?,
            locale: LocaleTag::new("es-MX")?,
        },
    );
    crate::SqliteOrganizationBranchStore::new(pool.clone())
        .insert_organization(
            &organization,
            "2026-10-30T00:00:00Z",
            "2026-10-30T00:00:00Z",
        )
        .await?;
    Ok(())
}

#[tokio::test]
async fn snapshot_includes_committed_wal_and_restores_durably()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("source.sqlite");
    let pool = crate::open(&source, Duration::from_secs(5)).await?;
    crate::migrate(&pool).await?;
    sqlx::query("PRAGMA wal_autocheckpoint=0")
        .execute(&pool)
        .await?;
    insert_organization(&pool, "1").await?;
    assert!(std::fs::metadata(directory.path().join("source.sqlite-wal"))?.len() > 32);
    let path = directory.path().join("snapshot.sqlite");
    let backup = create_backup(&pool, &path).await?;
    let manifest_digest = backup.digest();
    drop(backup);
    assert!(matches!(
        verify_existing_backup(&path, [0; 32]).await,
        Err(BackupError::Rejected)
    ));
    let backup = verify_existing_backup(&path, manifest_digest).await?;
    insert_organization(&pool, "2").await?;
    let restored = directory.path().join("restored.sqlite");
    restore_to_new_file(&backup, &restored).await?;
    assert_eq!(backup.digest(), digest_file(File::open(&restored)?)?);
    let reopened = crate::open(&restored, Duration::from_secs(5)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization")
            .fetch_one(&reopened)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization")
            .fetch_one(&pool)
            .await?,
        2
    );
    reopened.close().await;
    pool.close().await;
    let final_open = crate::open(&restored, Duration::from_secs(5)).await?;
    crate::migrate(&final_open).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organization")
            .fetch_one(&final_open)
            .await?,
        1
    );
    final_open.close().await;
    Ok(())
}

#[tokio::test]
async fn backup_and_restore_never_overwrite_existing_destinations()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let pool = crate::open(
        &directory.path().join("source.sqlite"),
        Duration::from_secs(5),
    )
    .await?;
    crate::migrate(&pool).await?;
    let path = directory.path().join("snapshot.sqlite");
    let backup = create_backup(&pool, &path).await?;
    for bytes in [b"existing installation".as_slice(), b"".as_slice()] {
        let target = directory.path().join("existing.sqlite");
        if !target.exists() {
            std::fs::write(&target, bytes)?;
        }
        assert!(matches!(
            create_backup(&pool, &target).await,
            Err(BackupError::DestinationExists)
        ));
        assert_eq!(
            restore_to_new_file(&backup, &target).await,
            Err(BackupError::DestinationExists)
        );
        assert_eq!(std::fs::read(&target)?, bytes);
        // Solo se borra un archivo propiedad de esta fixture temporal, no datos reales.
        std::fs::remove_file(&target)?;
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn orphaned_auxiliaries_prevent_backup_or_restore_without_deleting_them()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let pool = crate::open(
        &directory.path().join("source.sqlite"),
        Duration::from_secs(5),
    )
    .await?;
    crate::migrate(&pool).await?;
    let backup = create_backup(&pool, &directory.path().join("snapshot.sqlite")).await?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let target = directory.path().join(format!("target{suffix}.sqlite"));
        let mut sidecar = target.as_os_str().to_os_string();
        sidecar.push(suffix);
        std::fs::write(Path::new(&sidecar), b"existing auxiliary")?;
        assert!(matches!(
            create_backup(&pool, &target).await,
            Err(BackupError::DestinationExists)
        ));
        assert_eq!(
            restore_to_new_file(&backup, &target).await,
            Err(BackupError::DestinationExists)
        );
        assert!(!target.exists());
        assert_eq!(std::fs::read(Path::new(&sidecar))?, b"existing auxiliary");
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn changed_snapshot_is_not_restored_or_activated() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let pool = crate::open(
        &directory.path().join("source.sqlite"),
        Duration::from_secs(5),
    )
    .await?;
    crate::migrate(&pool).await?;
    let path = directory.path().join("snapshot.sqlite");
    let backup = create_backup(&pool, &path).await?;
    // Cambio con SQLite válido: la integridad física no basta, debe cambiar el digest.
    let connection_options = SqliteConnectOptions::new().filename(&path);
    let mut modified = SqliteConnection::connect_with(&connection_options).await?;
    sqlx::query("PRAGMA user_version=7")
        .execute(&mut modified)
        .await?;
    modified.close().await?;
    let destination = directory.path().join("must-not-exist.sqlite");
    assert_eq!(
        restore_to_new_file(&backup, &destination).await,
        Err(BackupError::Rejected)
    );
    assert!(!destination.exists());
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn future_or_changed_migration_history_cannot_be_published_as_backup()
-> Result<(), Box<dyn std::error::Error>> {
    for statement in [
        "UPDATE _sqlx_migrations SET version=99 WHERE version=(SELECT MAX(version) FROM _sqlx_migrations)",
        "UPDATE _sqlx_migrations SET checksum=x'00' WHERE version=(SELECT MAX(version) FROM _sqlx_migrations)",
    ] {
        let directory = tempfile::tempdir()?;
        let pool = crate::open(
            &directory.path().join("source.sqlite"),
            Duration::from_secs(5),
        )
        .await?;
        crate::migrate(&pool).await?;
        sqlx::query(statement).execute(&pool).await?;
        assert!(matches!(
            create_backup(&pool, &directory.path().join("rejected.sqlite")).await,
            Err(BackupError::Rejected)
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations")
                .fetch_one(&pool)
                .await?,
            17
        );
        pool.close().await;
    }
    Ok(())
}
