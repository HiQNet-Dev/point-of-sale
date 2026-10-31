//! Pruebas del composition root: modos, persistencia y límites de autorización.
use std::error::Error;

use super::*;
use crate::dto::{
    BootstrapInstallRequestDto, CreateProductRequest, LoginRequestDto, PageRequest,
    SelectBranchRequestDto,
};

fn page() -> PageRequest {
    PageRequest {
        limit: 10,
        offset: 0,
    }
}

fn select_branch(value: &str) -> SelectBranchRequestDto {
    SelectBranchRequestDto {
        branch_id: value.to_owned(),
    }
}

fn new_product(sku: &str, price: &str) -> CreateProductRequest {
    CreateProductRequest {
        sku: sku.to_owned(),
        name: "Producto de prueba".to_owned(),
        unit_price: price.to_owned(),
        category_id: None,
        barcode: None,
        description: None,
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
        assert_eq!(
            created.schema_version,
            u32::try_from(pos_storage_sqlite::approved_schema_version())?
        );
        assert_eq!(created.applied_migrations, 17);
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
fn only_one_desktop_runtime_can_open_an_installation() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let first = LocalState::open_installation(directory.path()).await?;
        assert!(matches!(
            LocalState::open_installation(directory.path()).await,
            Err(StartupError::InstallationInUse)
        ));
        first.shutdown().await;
        let second = LocalState::open_installation(directory.path()).await?;
        assert_eq!(second.installation_status()?.state, "REOPENED");
        second.shutdown().await;
        Ok(())
    })
}

#[test]
fn changed_checksum_is_rejected_and_startup_releases_the_lock() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join(DATABASE_FILE_NAME);
        let pool = pos_storage_sqlite::open(&database, Duration::from_secs(5)).await?;
        pos_storage_sqlite::migrate(&pool).await?;
        sqlx::query("UPDATE _sqlx_migrations SET checksum = x'00'")
            .execute(&pool)
            .await?;
        pool.close().await;
        for _ in 0..2 {
            assert!(matches!(
                LocalState::open_installation(directory.path()).await,
                Err(StartupError::Installation(
                    InstallationError::MigrationFailed
                ))
            ));
        }
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
                approved: 17
            }))
        ));
        Ok(())
    })
}

#[test]
fn product_mode_rejects_every_lab_command() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = tempfile::tempdir()?;
        let state = LocalState::open_installation(directory.path()).await?;
        assert_eq!(
            state
                .create_product(new_product("CAFE-2", "9.9900"))
                .await
                .err(),
            Some(IpcError::OperationNotAvailable)
        );
        assert_eq!(
            state.products(page()).await.err(),
            Some(IpcError::OperationNotAvailable)
        );
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn authentication_commands_are_gated_by_mode_and_session() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        // El laboratorio no tiene identidad de producto.
        let lab = LocalState::synthetic().await?;
        for result in [
            lab.auth_me().await.err(),
            lab.auth_logout().await.err(),
            lab.auth_select_branch(select_branch("00000000-0000-0000-0000-000000000001"))
                .await
                .err(),
            lab.auth_list_branches().await.err(),
        ] {
            assert_eq!(result, Some(IpcError::OperationNotAvailable));
        }
        lab.shutdown().await;

        // Una instalación sin sesión no autoriza nada y el login llega al caso
        // de uso real, que responde con el error genérico.
        let directory = tempfile::tempdir()?;
        let state = LocalState::open_installation(directory.path()).await?;
        assert_eq!(state.auth_me().await.err(), Some(IpcError::Unauthorized));
        assert_eq!(state.auth_logout().await, Ok(()));
        assert_eq!(
            state.auth_list_branches().await.err(),
            Some(IpcError::Unauthorized)
        );
        let login = LoginRequestDto {
            email: "nadie@hiqnet.test".to_owned(),
            password: "no-importa".to_owned(),
        };
        assert_eq!(
            state.auth_login(login).await.err(),
            Some(IpcError::Unauthorized)
        );
        // Sin sesión no se crea ninguna fila.
        let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_session")
            .fetch_one(&state.product_installation()?.pool)
            .await?;
        assert_eq!(sessions, 0);
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn first_installation_is_gated_by_platform_custody() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let lab = LocalState::synthetic().await?;
        assert_eq!(
            lab.bootstrap_status().await.err(),
            Some(IpcError::OperationNotAvailable)
        );
        lab.shutdown().await;

        let directory = tempfile::tempdir()?;
        let state = LocalState::open_installation(directory.path()).await?;
        let status = state.bootstrap_status().await?;
        #[cfg(windows)]
        assert_eq!(status.state, "UNSUPPORTED_PLATFORM");
        #[cfg(unix)]
        assert_eq!(status.state, "AWAITING_AUTHORIZATION");
        assert!(status.expires_at.is_none());

        let installed = state
            .bootstrap_install(BootstrapInstallRequestDto {
                business_name: "Café Central".to_owned(),
                business_code: None,
                administrator_name: "Administradora".to_owned(),
                email: "admin@cafe.test".to_owned(),
                password: "contraseña-larga-2k".to_owned(),
                currency: Some("MXN".to_owned()),
            })
            .await;
        #[cfg(windows)]
        assert_eq!(installed.err(), Some(IpcError::OperationNotAvailable));
        #[cfg(unix)]
        assert_eq!(installed.err(), Some(IpcError::Unauthorized));

        // Ningún intento sin autorización crea identidad.
        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pos_user")
            .fetch_one(&state.product_installation()?.pool)
            .await?;
        assert_eq!(users, 0);
        assert_eq!(state.bootstrap_status().await?.state, status.state);
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn catalog_commands_require_a_session_and_reject_the_laboratory() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let lab = LocalState::synthetic().await?;
        assert_eq!(
            lab.catalog_list_products(page()).await.err(),
            Some(IpcError::OperationNotAvailable)
        );
        assert_eq!(
            lab.catalog_create_category(crate::dto::NewCategoryRequestDto {
                code: "bebidas".to_owned(),
                name: "Bebidas".to_owned(),
            })
            .await
            .err(),
            Some(IpcError::OperationNotAvailable)
        );
        lab.shutdown().await;

        let directory = tempfile::tempdir()?;
        let state = LocalState::open_installation(directory.path()).await?;
        for result in [
            state.catalog_list_categories(page()).await.err(),
            state.catalog_list_products(page()).await.err(),
            state
                .catalog_get_product(crate::dto::CatalogItemRequestDto {
                    id: "00000000-0000-0000-0000-000000000001".to_owned(),
                })
                .await
                .err(),
            state
                .catalog_create_product(new_product("CAFE-1", "1.0000"))
                .await
                .err(),
        ] {
            assert_eq!(result, Some(IpcError::Unauthorized));
        }
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn create_product_persists_with_exact_price_and_audit() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        let created = state
            .create_product(new_product("CAFE-2", "9.9900"))
            .await?;
        assert_eq!(created.sku, "CAFE-2");
        assert_eq!(created.unit_price, "9.9900");
        assert_eq!(created.version, "0");

        let products = state.products(page()).await?;
        assert_eq!(products.len(), 2);
        assert!(products.iter().any(|product| product.sku == "CAFE-2"));

        // La auditoría de la nueva escritura comparte transacción con la fila.
        let audits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE action = 'product.created' AND entity_id = ?",
        )
        .bind(&created.id)
        .fetch_one(state.pool()?)
        .await?;
        assert_eq!(audits, 1);
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn duplicate_sku_is_a_conflict_and_invalid_price_is_rejected() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let state = LocalState::synthetic().await?;
        assert_eq!(
            state
                .create_product(new_product("CAFE-1", "9.9900"))
                .await
                .err(),
            Some(IpcError::Conflict)
        );
        assert_eq!(
            state
                .create_product(new_product("CAFE-3", "9.99999"))
                .await
                .err(),
            Some(IpcError::ValidationError)
        );
        assert_eq!(
            state.create_product(new_product("", "1.0000")).await.err(),
            Some(IpcError::ValidationError)
        );
        state.shutdown().await;
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
