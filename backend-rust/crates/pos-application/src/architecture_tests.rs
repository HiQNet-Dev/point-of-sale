//! Controles automáticos de la arquitectura hexagonal.
//!
//! Verifican la dirección de dependencias sin añadir herramientas externas:
//! `pos-core` no conoce infraestructura, `pos-application` no conoce SQLite y el
//! adaptador sí depende de la capa de aplicación. Se ejecutan con el resto de la
//! suite, de modo que una violación rompe las pruebas antes que el diseño.
#![allow(clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("../.."), Path::to_path_buf)
}

/// Extrae únicamente el bloque `[dependencies]`, ignorando dev-dependencies,
/// que sí pueden usar utilidades de prueba.
fn runtime_dependencies(manifest: &str) -> &str {
    let start = manifest.find("[dependencies]").unwrap_or(0);
    let rest = &manifest[start..];
    let end = rest.find("\n[").map_or(rest.len(), |offset| offset + 1);
    &rest[..end]
}

fn read(relative: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(workspace_root().join(relative))?)
}

fn collect_rust_files(directory: &Path, found: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_files(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

#[test]
fn domain_crate_has_no_infrastructure_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = read("crates/pos-core/Cargo.toml")?;
    let runtime = runtime_dependencies(&manifest);
    for forbidden in ["sqlx", "tokio", "axum", "tauri", "reqwest"] {
        assert!(
            !runtime.contains(forbidden),
            "pos-core no puede depender de {forbidden}: el dominio desconoce la infraestructura"
        );
    }

    let mut files = Vec::new();
    collect_rust_files(&workspace_root().join("crates/pos-core/src"), &mut files)?;
    for file in files {
        let source = fs::read_to_string(&file)?;
        for forbidden in [
            "sqlx::",
            "use tokio::",
            "use axum::",
            "pos_storage_sqlite",
            "pos_application",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} no puede referenciar {forbidden}",
                file.display()
            );
        }
    }
    Ok(())
}

#[test]
fn application_crate_depends_only_on_the_domain() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = read("crates/pos-application/Cargo.toml")?;
    let runtime = runtime_dependencies(&manifest);
    for forbidden in [
        "sqlx",
        "tokio",
        "axum",
        "tauri",
        "pos-storage-sqlite",
        "pos-local",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "pos-application no puede depender de {forbidden}"
        );
    }
    assert!(
        manifest.contains("pos-core"),
        "pos-application debe apoyarse en el dominio"
    );

    let mut files = Vec::new();
    collect_rust_files(
        &workspace_root().join("crates/pos-application/src"),
        &mut files,
    )?;
    for file in files {
        if file.ends_with("architecture_tests.rs") {
            // El propio guard nombra los patrones prohibidos.
            continue;
        }
        let source = fs::read_to_string(&file)?;
        for forbidden in ["sqlx::", "use tokio::", "pos_storage_sqlite"] {
            assert!(
                !source.contains(forbidden),
                "{} no puede referenciar {forbidden}",
                file.display()
            );
        }
    }
    Ok(())
}

#[test]
fn infrastructure_depends_inward() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = read("crates/pos-storage-sqlite/Cargo.toml")?;
    assert!(
        manifest.contains("pos-application"),
        "el adaptador SQLite implementa los puertos de aplicación"
    );
    assert!(
        manifest.contains("pos-core"),
        "el adaptador SQLite usa el dominio, nunca al revés"
    );
    Ok(())
}

#[test]
fn entrypoint_does_not_own_business_rules() -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_rust_files(&workspace_root().join("apps/pos-local/src"), &mut files)?;
    for file in files {
        if file
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("_tests.rs"))
        {
            // Las pruebas pueden construir fixtures SQL; las reglas se verifican
            // en el código de producción del entrypoint.
            continue;
        }
        let source = fs::read_to_string(&file)?;
        for forbidden in [
            "sqlx::query",
            "INSERT INTO",
            "UPDATE sale",
            "inventory_ledger",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} no puede contener reglas de negocio o SQL: {forbidden}",
                file.display()
            );
        }
    }
    Ok(())
}
