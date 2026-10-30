//! Aplicación Tauri aislada. Nunca abre bases existentes ni expone HTTP.
mod bindings;
mod command_names;
mod commands;
mod composition;
mod dto;
#[cfg(test)]
mod ipc_tests;

fn main() {
    if let Err(error) = execute() {
        eprintln!("No se pudo iniciar el laboratorio de escritorio: {error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|argument| argument == "--export-bindings") {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../frontend/src/shared/rust-local/desktop.generated.ts");
        bindings::export(&path)?;
        return Ok(());
    }
    if !std::env::args().any(|argument| argument == "--synthetic-lab") {
        return Err(
            "se requiere --synthetic-lab; no hay configuración productiva autorizada".into(),
        );
    }
    let state = tauri::async_runtime::block_on(composition::DesktopState::synthetic())?;
    eprintln!(
        "Laboratorio sintético: {} comandos de solo lectura",
        command_names::COMMAND_NAMES.len()
    );
    let registry = commands::registry::<tauri::Wry>();
    let app = tauri::Builder::default()
        .manage(state)
        .invoke_handler(registry.invoke_handler())
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            use tauri::Manager;
            tauri::async_runtime::block_on(handle.state::<composition::DesktopState>().shutdown());
        }
    });
    Ok(())
}
