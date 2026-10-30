//! Pruebas del dispatcher Tauri con MockRuntime: no equivalen a E2E gráfico.
use std::error::Error;
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    webview::InvokeRequest,
};

#[test]
fn registered_handlers_use_application_and_reject_invalid_pages() -> Result<(), Box<dyn Error>> {
    let state = tauri::async_runtime::block_on(crate::composition::DesktopState::synthetic())?;
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(crate::commands::registry::<tauri::test::MockRuntime>().invoke_handler())
        .build(tauri::generate_context!())?;
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build()?;
    let url: tauri::Url = "http://tauri.localhost/rust-lab.html".parse()?;
    let call = |command: &str, body: serde_json::Value| {
        tauri::test::get_ipc_response(
            &window,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: url.clone(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
    };
    for command in crate::command_names::COMMAND_NAMES {
        let response = call(
            command,
            serde_json::json!({"request": {"limit":20,"offset":0}}),
        )
        .map_err(|_| "handler rechazado")?
        .deserialize::<serde_json::Value>()?;
        assert_eq!(response.as_array().map(Vec::len), Some(1));
        if *command == "lab_list_products" {
            assert_eq!(response[0]["unitPrice"], "12.3456");
            assert!(response[0].get("unit_price").is_none());
        }
    }
    assert_eq!(
        call(
            "lab_list_products",
            serde_json::json!({"request":{"limit":0,"offset":0}})
        )
        .err(),
        Some(serde_json::json!({"code":"VALIDATION_ERROR"}))
    );
    assert!(
        call(
            "lab_list_products",
            serde_json::json!({"request":{"limit":20,"offset":0,"organizationId":"foreign"}})
        )
        .is_err()
    );
    assert!(call("provision_initial_installation", serde_json::json!({})).is_err());
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
