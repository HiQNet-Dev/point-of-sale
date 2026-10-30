//! Exportación determinista; normaliza solo formato, nunca tipos ni firmas.
use std::{error::Error, path::Path};

/// Genera bindings desde el mismo registro que utiliza el dispatcher real.
pub fn export(path: &Path) -> Result<(), Box<dyn Error>> {
    crate::commands::registry::<tauri::test::MockRuntime>()
        .export(specta_typescript::Typescript::default(), path)?;
    let source = std::fs::read_to_string(path)?;
    let lines = source
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{}\n", lines.trim_end()))?;
    Ok(())
}
