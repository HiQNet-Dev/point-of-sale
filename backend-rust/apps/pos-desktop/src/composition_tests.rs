//! Pruebas del composition root: modos, persistencia y límites de autorización.
use std::error::Error;

use super::*;
use crate::dto::PageRequest;

fn page() -> PageRequest {
    PageRequest {
        limit: 10,
        offset: 0,
    }
}

#[test]
fn branch_principal_cannot_enumerate_the_organization() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        let visible = state.branches().await?;
        let branch = pos_core::organization_branch::EntityId::parse(&visible[0].id)?;
        let principal =
            pos_lab::authenticate_candidate_session(state.pool()?, Some(&branch)).await?;
        let reader = SqliteOrganizationBranchStore::new(state.pool()?.clone());
        assert!(matches!(
            list_authorized_branches(&reader, &principal).await,
            Err(pos_core::organization_branch::ApplicationError::Forbidden)
        ));
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn permission_removal_is_effective_on_next_call() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        sqlx::query("DELETE FROM role_permission WHERE permission_code = 'product.read'")
            .execute(state.pool()?)
            .await?;
        assert_eq!(
            state.products(page()).await.err(),
            Some(IpcError::Forbidden)
        );
        assert!(!state.categories(page()).await?.is_empty());
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn expired_session_is_not_authorized() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        sqlx::query("UPDATE local_session SET created_at = '2026-10-30T08:00:00Z', expires_at = '2026-10-30T08:59:00Z'").execute(state.pool()?).await?;
        assert_eq!(state.branches().await.err(), Some(IpcError::Unauthorized));
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn branch_queries_never_return_other_organization() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        sqlx::query("INSERT INTO organization SELECT '00000000-0000-0000-0000-000000000999', 'foreign', name, currency, timezone, locale, stock_negative_policy, status, version, created_at, updated_at FROM organization LIMIT 1")
            .execute(state.pool()?).await?;
        sqlx::query("INSERT INTO branch SELECT '00000000-0000-0000-0000-000000000998', '00000000-0000-0000-0000-000000000999', 'foreign-branch', name, status, timezone_override, version, created_at, updated_at FROM branch LIMIT 1")
            .execute(state.pool()?).await?;
        let branches = state.branches().await?;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].code, "centro");
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn authorization_is_revalidated_after_revocation() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        assert!(!state.products(page()).await?.is_empty());
        sqlx::query(
            "UPDATE local_session SET state = 'REVOKED', revoked_at = '2026-10-30T10:00:00Z'",
        )
        .execute(state.pool()?)
        .await?;
        assert_eq!(
            state.products(page()).await.err(),
            Some(IpcError::Unauthorized)
        );
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn lab_mode_has_no_persistent_installation() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        assert_eq!(
            state.installation_status().err(),
            Some(IpcError::OperationNotAvailable)
        );
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn installation_persists_across_reopen() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let first = LocalState::open_installation(directory.path()).await?;
        let created = first.installation_status()?;
        assert_eq!(created.state, "FIRST_INSTALL");
        assert_eq!(created.schema_version, 1);
        assert_eq!(created.applied_migrations, 1);
        first.shutdown().await;

        // El archivo debe existir después del cierre: la instalación es durable.
        assert!(directory.path().join(DATABASE_FILE_NAME).is_file());

        let second = LocalState::open_installation(directory.path()).await?;
        let reopened = second.installation_status()?;
        assert_eq!(reopened.state, "REOPENED");
        assert_eq!(reopened.schema_version, created.schema_version);
        assert_eq!(reopened.applied_migrations, created.applied_migrations);
        // Sin identidad autorizada, el modo instalación no sirve datos de negocio.
        assert_eq!(
            second.branches().await.err(),
            Some(IpcError::OperationNotAvailable)
        );
        second.shutdown().await;
        Ok(())
    })
}

#[test]
fn newer_schema_is_rejected_at_startup() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join(DATABASE_FILE_NAME);
        let pool = pos_storage_sqlite::open(&database, Duration::from_secs(5)).await?;
        pos_storage_sqlite::migrate(&pool).await?;
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
             VALUES (99, 'future', 1, x'00', 0)",
        )
        .execute(&pool)
        .await?;
        pool.close().await;

        assert!(matches!(
            LocalState::open_installation(directory.path()).await,
            Err(StartupError::Installation(InstallationError::SchemaAhead {
                found: 99,
                approved: 1
            }))
        ));
        Ok(())
    })
}

#[test]
fn installation_directory_must_not_be_a_file() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let file = directory.path().join("archivo");
        std::fs::write(&file, b"no es un directorio")?;
        assert!(matches!(
            LocalState::open_installation(&file).await,
            Err(StartupError::InvalidInstallationDirectory)
        ));
        Ok(())
    })
}
