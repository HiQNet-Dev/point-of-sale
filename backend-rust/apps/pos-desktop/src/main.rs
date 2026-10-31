//! Aplicación de escritorio Local. Nunca abre HTTP ni una instalación implícita.
//!
//! Modos explícitos (ver [`arguments`]):
//!
//! - `--synthetic-lab`: laboratorio efímero con fixtures sintéticas.
//! - `--installation <directorio>`: instalación persistente con migraciones
//!   aprobadas; expone únicamente su estado.
//! - `--installation-default`: la misma instalación, en el directorio privado
//!   que el sistema operativo asigna a la aplicación.
//! - `--export-bindings`: regenera los contratos TypeScript.
mod arguments;
mod auth;
mod bindings;
mod bootstrap;
mod catalog;
mod command_names;
mod commands;
mod composition;
mod dto;
mod installation_dir;
#[cfg(test)]
mod ipc_tests;
mod purchasing;

use std::error::Error;

use arguments::{ArgumentError, Invocation};
use composition::LocalState;

fn main() {
    if let Err(error) = execute() {
        eprintln!("No se pudo iniciar HiQNet POS Local: {error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), Box<dyn Error>> {
    match arguments::parse(std::env::args().skip(1)).map_err(usage)? {
        Invocation::ExportBindings => {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../frontend/src/shared/rust-local/desktop.generated.ts");
            bindings::export(&path)?;
            bindings::export_purchasing(&path.with_file_name("purchasing.generated.ts"))?;
            Ok(())
        }
        Invocation::SyntheticLab => {
            let state = tauri::async_runtime::block_on(LocalState::synthetic())?;
            eprintln!(
                "Laboratorio sintético: {} comandos locales",
                command_names::COMMAND_NAMES.len()
            );
            run_desktop(state)
        }
        Invocation::Installation(directory) => {
            let state = tauri::async_runtime::block_on(LocalState::open_installation(&directory))?;
            run_desktop(state)
        }
        Invocation::InstallationDefault => {
            // El directorio lo elige el sistema operativo; JavaScript nunca lo
            // suministra. Si no existe se crea bajo el perfil del usuario.
            let directory = installation_dir::default_data_directory()?;
            let state = tauri::async_runtime::block_on(LocalState::open_installation(&directory))?;
            run_desktop(state)
        }
    }
}

/// Construye la única ventana local y cierra el pool al salir.
fn run_desktop(state: LocalState) -> Result<(), Box<dyn Error>> {
    let registry = commands::registry::<tauri::Wry>();
    let app = tauri::Builder::default()
        .manage(state)
        .invoke_handler(registry.invoke_handler())
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            use tauri::Manager;
            tauri::async_runtime::block_on(handle.state::<LocalState>().shutdown());
        }
    });
    Ok(())
}

/// Traduce un error de uso a un mensaje estático y accionable.
fn usage(error: ArgumentError) -> &'static str {
    match error {
        ArgumentError::MissingMode => {
            "indique --synthetic-lab, --installation <directorio> o --installation-default; no hay configuración productiva implícita"
        }
        ArgumentError::ConflictingModes => "indique un único modo de arranque",
        ArgumentError::MissingInstallationDirectory => {
            "falta el directorio después de --installation"
        }
        ArgumentError::EmptyInstallationDirectory => {
            "el directorio de --installation no puede estar vacío"
        }
        ArgumentError::UnknownArgument => "argumento no reconocido",
    }
}
