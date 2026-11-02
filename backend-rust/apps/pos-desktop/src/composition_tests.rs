//! Pruebas del composition root: modos, persistencia y límites de autorización.
use std::{error::Error, path::Path};

use pos_core::identity::{ProvisioningAuthorization, ProvisioningPurpose};
use pos_credentials::ProvisioningToken;
use pos_storage_sqlite::{
    native_authorization::NATIVE_AUTHORIZATION_FILE, provisioning::SqliteProvisioningStore,
};

use super::*;
use crate::dto::{
    BootstrapInstallRequestDto, CashRegisterListRequestDto, CashShiftListRequestDto,
    CloseCashShiftRequestDto, CreateBranchRequestDto, CreateProductRequest, LoginRequestDto,
    NewCashRegisterRequestDto, OpenCashShiftRequestDto, PageRequest, SelectBranchRequestDto,
    TimezoneOverrideRequestDto, UpdateBranchRequestDto, UpdateOrganizationRequestDto,
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
        let directory = crate::test_support::Fixture::new()?;
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
        let directory = crate::test_support::Fixture::new()?;
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
        let directory = crate::test_support::Fixture::new()?;
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
        let directory = crate::test_support::Fixture::new()?;
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
        let directory = crate::test_support::Fixture::new()?;
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
        let directory = crate::test_support::Fixture::new()?;
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

        // Unix y Windows resuelven la custodia de forma distinta pero producen el
        // mismo estado: sin autorización entregada no se instala nada.
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        let status = state.bootstrap_status().await?;
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
fn first_installation_from_a_native_authorization_creates_the_administrator()
-> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        let pool = state.product_installation()?.pool.clone();
        deliver_authorization(directory.path(), &pool).await?;

        // El asistente ve un derecho vivo con su ventana real.
        let ready = state.bootstrap_status().await?;
        assert_eq!(ready.state, "READY");
        assert!(ready.expires_at.is_some());

        state
            .bootstrap_install(BootstrapInstallRequestDto {
                business_name: "Café Central".to_owned(),
                business_code: None,
                administrator_name: "Administradora".to_owned(),
                email: "admin@cafe.test".to_owned(),
                password: "contraseña-larga-2k".to_owned(),
                currency: Some("MXN".to_owned()),
            })
            .await?;
        assert_eq!(state.bootstrap_status().await?.state, "ALREADY_PROVISIONED");

        // El administrador creado se autentica y obtiene sus permisos reales.
        let session = state
            .auth_login(LoginRequestDto {
                email: "admin@cafe.test".to_owned(),
                password: "contraseña-larga-2k".to_owned(),
            })
            .await?;
        assert_eq!(session.display_name, "Administradora");
        assert!(session.permissions.contains(&"product.create".to_owned()));
        assert_eq!(state.auth_me().await?.email, "admin@cafe.test");

        state.auth_logout().await?;
        assert_eq!(state.auth_me().await.err(), Some(IpcError::Unauthorized));

        // Reapertura: la identidad persiste y el derecho ya no se puede reutilizar.
        state.shutdown().await;
        let reopened = LocalState::open_installation(directory.path()).await?;
        assert_eq!(
            reopened.bootstrap_status().await?.state,
            "ALREADY_PROVISIONED"
        );
        let again = reopened
            .auth_login(LoginRequestDto {
                email: "admin@cafe.test".to_owned(),
                password: "contraseña-larga-2k".to_owned(),
            })
            .await?;
        assert_eq!(again.user_id, session.user_id);
        reopened.shutdown().await;
        Ok(())
    })
}

/// Emisor administrativo: instalación provisional, derecho vivo y archivo privado.
async fn deliver_authorization(
    directory: &Path,
    pool: &sqlx::SqlitePool,
) -> Result<(), Box<dyn Error>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let now = i64::try_from(now)?;
    let installation_id = EntityId::parse("00000000-0000-0000-0000-00000000c0f0")?;
    let authorization_id = EntityId::parse("00000000-0000-0000-0000-00000000c1a0")?;
    let event_id = EntityId::parse("00000000-0000-0000-0000-00000000c1e0")?;
    let token = ProvisioningToken::generate()?;

    let store = SqliteProvisioningStore::new(pool.clone());
    store
        .ensure_provisional_installation(&installation_id, "local:desktop-test", now)
        .await?;
    let authorization = ProvisioningAuthorization::issue(
        authorization_id.clone(),
        installation_id.clone(),
        ProvisioningPurpose::Bootstrap,
        &token.digest().hex(),
        now,
        now + 600,
    )?;
    store.issue(&authorization, &event_id, now).await?;

    let contents = format!(
        "installation_id={}\nauthorization_id={}\ntoken={}\n",
        installation_id.as_str(),
        authorization_id.as_str(),
        token.expose()
    );
    write_private_authorization(directory, &contents)
}

#[cfg(unix)]
fn write_private_authorization(directory: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join(NATIVE_AUTHORIZATION_FILE))?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn write_private_authorization(directory: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    pos_windows_security::create_private_file(
        directory,
        NATIVE_AUTHORIZATION_FILE,
        contents.as_bytes(),
    )?;
    Ok(())
}

#[test]
fn business_configuration_requires_a_session_and_rejects_the_laboratory()
-> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let lab = LocalState::synthetic().await?;
        assert_eq!(
            lab.organization_get().await.err(),
            Some(IpcError::OperationNotAvailable)
        );
        assert_eq!(
            lab.branch_list().await.err(),
            Some(IpcError::OperationNotAvailable)
        );
        assert_eq!(
            lab.branch_get("00000000-0000-0000-0000-000000000001")
                .await
                .err(),
            Some(IpcError::OperationNotAvailable)
        );
        lab.shutdown().await;

        // Instalación sin sesión: ningún dato de negocio sale del proceso.
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        assert_eq!(
            state.organization_get().await.err(),
            Some(IpcError::Unauthorized)
        );
        assert_eq!(
            state.branch_list().await.err(),
            Some(IpcError::Unauthorized)
        );
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn business_configuration_round_trip_survives_a_restart() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        deliver_authorization(directory.path(), &state.product_installation()?.pool).await?;
        install_and_login(&state).await?;

        // Configuración inicial derivada del bootstrap.
        let organization = state.organization_get().await?;
        assert_eq!(organization.name, "Café Central");
        assert_eq!(organization.code, "caf-central");
        assert_eq!(organization.currency, "MXN");
        assert_eq!(organization.status, "ACTIVE");
        assert_eq!(organization.negative_stock_policy, "STRICT");
        assert_eq!(organization.revision, "0");

        // Edición de los campos permitidos: nombre, zona, política.
        let updated = state
            .organization_update(UpdateOrganizationRequestDto {
                expected_revision: organization.revision.clone(),
                name: Some("Café Central Norte".to_owned()),
                timezone: Some("America/Tijuana".to_owned()),
                locale: None,
                negative_stock_policy: Some("ALLOW_NEGATIVE".to_owned()),
            })
            .await?;
        assert_eq!(updated.name, "Café Central Norte");
        assert_eq!(updated.timezone, "America/Tijuana");
        assert_eq!(updated.negative_stock_policy, "ALLOW_NEGATIVE");
        assert_eq!(updated.revision, "1");
        // Moneda y estado quedan intactos: el contrato no los expone.
        assert_eq!(updated.currency, "MXN");
        assert_eq!(updated.status, "ACTIVE");

        // Una revisión obsoleta no modifica nada.
        assert_eq!(
            state
                .organization_update(UpdateOrganizationRequestDto {
                    expected_revision: "0".to_owned(),
                    name: Some("Otro".to_owned()),
                    timezone: None,
                    locale: None,
                    negative_stock_policy: None,
                })
                .await
                .err(),
            Some(IpcError::Conflict)
        );
        assert_eq!(state.organization_get().await?.revision, "1");

        // La instalación nace sin sucursal: se configura la inicial.
        assert!(state.branch_list().await?.is_empty());
        let branch = state
            .branch_create(CreateBranchRequestDto {
                code: "centro".to_owned(),
                name: "Sucursal Centro".to_owned(),
                timezone: None,
            })
            .await?;
        assert_eq!(branch.code, "centro");
        assert!(branch.inherits_timezone);
        assert_eq!(branch.effective_timezone, "America/Tijuana");

        // La segunda sucursal la rechaza la capa de aplicación.
        assert_eq!(
            state
                .branch_create(CreateBranchRequestDto {
                    code: "norte".to_owned(),
                    name: "Sucursal Norte".to_owned(),
                    timezone: None,
                })
                .await
                .err(),
            Some(IpcError::Conflict)
        );

        // Triestado de zona horaria sobre la sucursal existente.
        let overridden = state
            .branch_update(UpdateBranchRequestDto {
                branch_id: branch.id.clone(),
                expected_revision: branch.revision.clone(),
                name: Some("Sucursal Centro Histórico".to_owned()),
                timezone: TimezoneOverrideRequestDto {
                    mode: "SET".to_owned(),
                    timezone: Some("America/Cancun".to_owned()),
                },
            })
            .await?;
        assert!(!overridden.inherits_timezone);
        assert_eq!(
            overridden.timezone_override.as_deref(),
            Some("America/Cancun")
        );
        assert_eq!(overridden.effective_timezone, "America/Cancun");
        assert_eq!(overridden.revision, "1");

        // `SET` sin zona horaria es ambiguo y se rechaza.
        assert_eq!(
            state
                .branch_update(UpdateBranchRequestDto {
                    branch_id: branch.id.clone(),
                    expected_revision: overridden.revision.clone(),
                    name: None,
                    timezone: TimezoneOverrideRequestDto {
                        mode: "SET".to_owned(),
                        timezone: None,
                    },
                })
                .await
                .err(),
            Some(IpcError::ValidationError)
        );

        // `INHERIT` elimina el override.
        let inherited = state
            .branch_update(UpdateBranchRequestDto {
                branch_id: branch.id.clone(),
                expected_revision: overridden.revision.clone(),
                name: None,
                timezone: TimezoneOverrideRequestDto {
                    mode: "INHERIT".to_owned(),
                    timezone: None,
                },
            })
            .await?;
        assert!(inherited.inherits_timezone);
        assert_eq!(inherited.effective_timezone, "America/Tijuana");

        // La lectura individual coincide con el listado.
        let listed = state.branch_list().await?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, branch.id);
        assert_eq!(
            state.branch_get(&branch.id).await?.name,
            "Sucursal Centro Histórico"
        );

        // Cierre y reapertura: la configuración persiste.
        state.auth_logout().await?;
        state.shutdown().await;
        let reopened = LocalState::open_installation(directory.path()).await?;
        reopened
            .auth_login(LoginRequestDto {
                email: "admin@cafe.test".to_owned(),
                password: "contraseña-larga-2k".to_owned(),
            })
            .await?;
        let persisted = reopened.organization_get().await?;
        assert_eq!(persisted.name, "Café Central Norte");
        assert_eq!(persisted.revision, "1");
        let persisted_branch = reopened.branch_list().await?;
        assert_eq!(persisted_branch.len(), 1);
        assert_eq!(persisted_branch[0].name, "Sucursal Centro Histórico");
        assert!(persisted_branch[0].inherits_timezone);
        reopened.shutdown().await;
        Ok(())
    })
}

#[test]
fn removing_organization_update_is_effective_on_the_next_call() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        let pool = state.product_installation()?.pool.clone();
        deliver_authorization(directory.path(), &pool).await?;
        install_and_login(&state).await?;
        assert_eq!(state.organization_get().await?.revision, "0");

        // Se retira el permiso en SQLite: la siguiente llamada ya no autoriza.
        sqlx::query("DELETE FROM role_permission WHERE permission_code = 'organization.update'")
            .execute(&pool)
            .await?;
        assert_eq!(
            state
                .organization_update(UpdateOrganizationRequestDto {
                    expected_revision: "0".to_owned(),
                    name: Some("Sin permiso".to_owned()),
                    timezone: None,
                    locale: None,
                    negative_stock_policy: None,
                })
                .await
                .err(),
            Some(IpcError::Forbidden)
        );
        // La lectura sigue autorizada por su propio permiso.
        assert_eq!(state.organization_get().await?.name, "Café Central");
        state.shutdown().await;
        Ok(())
    })
}

/// Instala el negocio y deja una sesión activa para el administrador.
async fn install_and_login(state: &LocalState) -> Result<(), Box<dyn Error>> {
    state
        .bootstrap_install(BootstrapInstallRequestDto {
            business_name: "Café Central".to_owned(),
            business_code: None,
            administrator_name: "Administradora".to_owned(),
            email: "admin@cafe.test".to_owned(),
            password: "contraseña-larga-2k".to_owned(),
            currency: Some("MXN".to_owned()),
        })
        .await?;
    state
        .auth_login(LoginRequestDto {
            email: "admin@cafe.test".to_owned(),
            password: "contraseña-larga-2k".to_owned(),
        })
        .await?;
    Ok(())
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

        let directory = crate::test_support::Fixture::new()?;
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
        let directory = crate::test_support::Fixture::new()?;
        let file = directory.path().join("archivo");
        std::fs::write(&file, b"no es un directorio")?;
        assert!(matches!(
            LocalState::open_installation(&file).await,
            Err(StartupError::InvalidInstallationDirectory)
        ));
        Ok(())
    })
}

#[test]
fn administration_commands_require_a_session_and_reject_the_laboratory()
-> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let lab = LocalState::synthetic().await?;
        for error in [
            lab.user_list().await.err(),
            lab.role_list().await.err(),
            lab.permission_list().await.err(),
            lab.membership_list().await.err(),
        ] {
            assert_eq!(error, Some(IpcError::OperationNotAvailable));
        }
        lab.shutdown().await;

        // El laboratorio tampoco puede crear identidades comerciales.
        let lab = LocalState::synthetic().await?;
        assert_eq!(
            lab.user_create(crate::dto::NewUserRequestDto {
                display_name: "Lab".into(),
                email: "lab@hiqnet.test".into(),
                password: "clave-de-laboratorio".into(),
            })
            .await
            .err(),
            Some(IpcError::OperationNotAvailable)
        );
        lab.shutdown().await;

        // Instalación sin sesión: ningún dato administrativo sale del proceso.
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        assert_eq!(state.user_list().await.err(), Some(IpcError::Unauthorized));
        assert_eq!(state.role_list().await.err(), Some(IpcError::Unauthorized));
        assert_eq!(
            state.membership_list().await.err(),
            Some(IpcError::Unauthorized)
        );
        assert_eq!(
            state
                .user_update(crate::dto::UpdateUserRequestDto {
                    user_id: "00000000-0000-0000-0000-000000000001".into(),
                    expected_revision: "0".into(),
                    display_name: None,
                    email: None,
                    status: None,
                })
                .await
                .err(),
            Some(IpcError::Unauthorized)
        );
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn cash_commands_require_a_session_and_reject_the_laboratory() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let lab = LocalState::synthetic().await?;
        for error in [
            lab.cash_register_list(cash_page()).await.err(),
            lab.cash_shift_get_open("00000000-0000-0000-0000-000000000001")
                .await
                .err(),
        ] {
            assert_eq!(error, Some(IpcError::OperationNotAvailable));
        }
        lab.shutdown().await;

        // Instalación sin sesión: ningún dato de caja sale del proceso.
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        for error in [
            state.cash_register_list(cash_page()).await.err(),
            state
                .cash_register_create(NewCashRegisterRequestDto {
                    code: "caja-1".to_owned(),
                    name: "Caja principal".to_owned(),
                })
                .await
                .err(),
            state
                .cash_shift_open(OpenCashShiftRequestDto {
                    cash_register_id: "00000000-0000-0000-0000-000000000001".to_owned(),
                    opening_amount: "100.0000".to_owned(),
                })
                .await
                .err(),
        ] {
            assert_eq!(error, Some(IpcError::Unauthorized));
        }
        state.shutdown().await;
        Ok(())
    })
}

#[test]
fn cash_register_and_shift_round_trip_survives_a_restart() -> Result<(), Box<dyn Error>> {
    tauri::async_runtime::block_on(async {
        let directory = crate::test_support::Fixture::new()?;
        let state = LocalState::open_installation(directory.path()).await?;
        let pool = state.product_installation()?.pool.clone();
        deliver_authorization(directory.path(), &pool).await?;
        install_and_login(&state).await?;

        let branch = state
            .branch_create(CreateBranchRequestDto {
                code: "centro".to_owned(),
                name: "Sucursal Centro".to_owned(),
                timezone: None,
            })
            .await?;
        state.auth_select_branch(select_branch(&branch.id)).await?;

        let register = state
            .cash_register_create(NewCashRegisterRequestDto {
                code: "caja-1".to_owned(),
                name: "Caja principal".to_owned(),
            })
            .await?;
        assert_eq!(register.revision, "0");
        assert_eq!(register.status, "ACTIVE");
        assert_eq!(
            state.cash_register_list(cash_page()).await?.len(),
            1,
            "la caja recién creada debe listarse"
        );

        // Código duplicado en la misma sucursal: conflicto, no un error interno.
        assert_eq!(
            state
                .cash_register_create(NewCashRegisterRequestDto {
                    code: "caja-1".to_owned(),
                    name: "Otra".to_owned(),
                })
                .await
                .err(),
            Some(IpcError::Conflict)
        );

        let shift = state
            .cash_shift_open(OpenCashShiftRequestDto {
                cash_register_id: register.id.clone(),
                opening_amount: "150.0000".to_owned(),
            })
            .await?;
        assert_eq!(shift.status, "OPEN");
        assert_eq!(shift.opening_amount, "150.0000");

        // Segunda apertura: la caja ya tiene un turno abierto.
        assert_eq!(
            state
                .cash_shift_open(OpenCashShiftRequestDto {
                    cash_register_id: register.id.clone(),
                    opening_amount: "10.0000".to_owned(),
                })
                .await
                .err(),
            Some(IpcError::Conflict)
        );

        let open = state
            .cash_shift_get_open(&register.id)
            .await?
            .ok_or("falta el turno abierto")?;
        assert_eq!(open.id, shift.id);

        let closed = state
            .cash_shift_close(CloseCashShiftRequestDto {
                cash_register_id: register.id.clone(),
                cash_shift_id: shift.id.clone(),
                expected_revision: shift.revision.clone(),
                closing_declared_amount: "150.0000".to_owned(),
            })
            .await?;
        assert_eq!(closed.status, "CLOSED");
        assert_eq!(closed.closing_declared_amount.as_deref(), Some("150.0000"));

        // La revisión quedó obsoleta: el cierre no es idempotente.
        assert_eq!(
            state
                .cash_shift_close(CloseCashShiftRequestDto {
                    cash_register_id: register.id.clone(),
                    cash_shift_id: shift.id.clone(),
                    expected_revision: shift.revision.clone(),
                    closing_declared_amount: "150.0000".to_owned(),
                })
                .await
                .err(),
            Some(IpcError::Conflict)
        );

        assert!(state.cash_shift_get_open(&register.id).await?.is_none());
        let history = state
            .cash_shift_list(CashShiftListRequestDto {
                cash_register_id: register.id.clone(),
                status: None,
                limit: 20,
                offset: 0,
            })
            .await?;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, "CLOSED");
        state.shutdown().await;

        // Reapertura: la sesión se perdió, pero caja y turno persisten.
        let reopened = LocalState::open_installation(directory.path()).await?;
        assert_eq!(
            reopened.cash_register_list(cash_page()).await.err(),
            Some(IpcError::Unauthorized)
        );
        reopened
            .auth_login(LoginRequestDto {
                email: "admin@cafe.test".to_owned(),
                password: "contraseña-larga-2k".to_owned(),
            })
            .await?;
        reopened
            .auth_select_branch(select_branch(&branch.id))
            .await?;
        let registers = reopened.cash_register_list(cash_page()).await?;
        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0].code, "caja-1");
        let reopened_history = reopened
            .cash_shift_list(CashShiftListRequestDto {
                cash_register_id: register.id.clone(),
                status: Some("CLOSED".to_owned()),
                limit: 20,
                offset: 0,
            })
            .await?;
        assert_eq!(reopened_history.len(), 1);
        assert_eq!(reopened_history[0].id, shift.id);
        reopened.shutdown().await;
        Ok(())
    })
}

fn cash_page() -> CashRegisterListRequestDto {
    CashRegisterListRequestDto {
        status: None,
        limit: 20,
        offset: 0,
    }
}
