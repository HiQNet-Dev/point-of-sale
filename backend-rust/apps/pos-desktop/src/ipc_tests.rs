//! Pruebas del dispatcher Tauri con MockRuntime: no equivalen a E2E gráfico.
use std::error::Error;
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    webview::InvokeRequest,
};

fn call(
    window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    url: &tauri::Url,
    command: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let response = tauri::test::get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.clone(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )?;
    response
        .deserialize::<serde_json::Value>()
        .map_err(|_| serde_json::json!({"code":"DESERIALIZATION_FAILED"}))
}

#[test]
fn registered_handlers_use_application_and_reject_invalid_pages() -> Result<(), Box<dyn Error>> {
    let state = tauri::async_runtime::block_on(crate::composition::LocalState::synthetic())?;
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(crate::commands::registry::<tauri::test::MockRuntime>().invoke_handler())
        .build(tauri::generate_context!())?;
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build()?;
    let url: tauri::Url = "http://tauri.localhost/rust-lab.html".parse()?;
    for command in crate::command_names::LAB_COMMAND_NAMES {
        let response = call(
            &window,
            &url,
            command,
            serde_json::json!({"request": {"limit":20,"offset":0}}),
        )
        .map_err(|_| "handler rechazado")?;
        assert_eq!(response.as_array().map(Vec::len), Some(1));
        if *command == "lab_list_products" {
            assert_eq!(response[0]["unitPrice"], "12.3456");
            assert!(response[0].get("unit_price").is_none());
        }
    }
    assert_eq!(
        call(
            &window,
            &url,
            "lab_list_products",
            serde_json::json!({"request":{"limit":0,"offset":0}})
        )
        .err(),
        Some(serde_json::json!({"code":"VALIDATION_ERROR"}))
    );
    assert!(
        call(
            &window,
            &url,
            "lab_list_products",
            serde_json::json!({"request":{"limit":20,"offset":0,"organizationId":"foreign"}})
        )
        .is_err()
    );
    // El laboratorio no expone estado de instalación persistente.
    assert_eq!(
        call(&window, &url, "installation_status", serde_json::json!({})).err(),
        Some(serde_json::json!({"code":"OPERATION_NOT_AVAILABLE"}))
    );
    assert!(
        call(
            &window,
            &url,
            "provision_initial_installation",
            serde_json::json!({})
        )
        .is_err()
    );
    let remote = tauri::test::get_ipc_response(
        &window,
        InvokeRequest {
            cmd: "lab_list_branches".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "https://untrusted.invalid/".parse()?,
            body: InvokeBody::Json(serde_json::json!({})),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    );
    assert!(
        remote.is_err(),
        "el origen remoto no tiene capabilities locales"
    );
    Ok(())
}

#[test]
fn installation_mode_exposes_only_its_status() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let state = tauri::async_runtime::block_on(crate::composition::LocalState::open_installation(
        directory.path(),
    ))?;
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(crate::commands::registry::<tauri::test::MockRuntime>().invoke_handler())
        .build(tauri::generate_context!())?;
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build()?;
    let url: tauri::Url = "http://tauri.localhost/rust-lab.html".parse()?;

    let status = call(&window, &url, "installation_status", serde_json::json!({}))
        .map_err(|error| format!("installation_status rechazado: {error}"))?;
    assert_eq!(status["state"], "FIRST_INSTALL");
    assert_eq!(status["schemaVersion"], 1);
    assert_eq!(status["appliedMigrations"], 1);
    assert!(status.get("databasePath").is_none());

    for command in crate::command_names::LAB_COMMAND_NAMES {
        assert_eq!(
            call(
                &window,
                &url,
                command,
                serde_json::json!({"request":{"limit":20,"offset":0}})
            )
            .err(),
            Some(serde_json::json!({"code":"OPERATION_NOT_AVAILABLE"}))
        );
    }
    Ok(())
}

#[test]
fn handlers_do_not_cross_hexagonal_layers() {
    let source = include_str!("commands.rs");
    for forbidden in ["sqlx", "Sqlite", "pos_storage", "pos_lab", "allow_all"] {
        assert!(
            !source.contains(forbidden),
            "handler acoplado a {forbidden}"
        );
    }
    let application = include_str!("../../../crates/pos-application/src/organization/queries.rs");
    for forbidden in ["sqlx", "tauri", "pos_storage"] {
        assert!(!application.contains(forbidden));
    }
    let installation = include_str!("../../../crates/pos-application/src/installation/mod.rs");
    for forbidden in ["sqlx", "tauri", "pos_storage"] {
        assert!(!installation.contains(forbidden));
    }
}

#[test]
fn generated_bindings_match_registered_commands() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("bindings.ts");
    crate::bindings::export(&target)?;
    let actual = std::fs::read_to_string(target)?.replace("\r\n", "\n");
    assert_eq!(
        actual,
        include_str!("../../../../frontend/src/shared/rust-local/desktop.generated.ts")
            .replace("\r\n", "\n")
    );
    for command in crate::command_names::COMMAND_NAMES {
        assert!(actual.contains(command));
    }
    Ok(())
}
