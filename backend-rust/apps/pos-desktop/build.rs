//! Configuración de recursos y permisos de los comandos locales.
#[path = "src/command_names.rs"]
mod command_names;

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(command_names::COMMAND_NAMES)),
    )
    .unwrap_or_else(|error| {
        eprintln!("Configuración de Tauri inválida: {error}");
        std::process::exit(1);
    });
}
