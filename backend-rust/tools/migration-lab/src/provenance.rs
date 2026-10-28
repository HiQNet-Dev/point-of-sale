//! Procedencia calculada desde fuentes del checkout; nunca desde hashes copiados.

use std::{collections::BTreeSet, fs, path::Path, process::Command};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{LabError, Result};

pub(crate) fn sha256(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes.as_ref());
    let mut hex = String::with_capacity(digest.len() * 2);
    for &byte in digest.iter() {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    hex
}

fn command(cwd: &Path, program: &str, args: &[&str]) -> Value {
    match Command::new(program).args(args).current_dir(cwd).output() {
        Ok(output) if output.status.success() => json!({
            "status": "OBSERVED", "stdout": String::from_utf8_lossy(&output.stdout).trim()
        }),
        Ok(output) => json!({
            "status": "NOT_RUN", "reason": String::from_utf8_lossy(&output.stderr).trim(),
            "exit_code": output.status.code()
        }),
        Err(error) => json!({"status": "NOT_RUN", "reason": error.to_string()}),
    }
}

/// Extrae identificadores sin confundir comentarios, cadenas ni cuerpos dollar-quoted con DDL.
fn tokens(sql: &str) -> Result<Vec<String>> {
    let bytes = sql.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                let mut depth = 1;
                while i < bytes.len() && depth > 0 {
                    if bytes[i..].starts_with(b"/*") {
                        depth += 1;
                        i += 2;
                    } else if bytes[i..].starts_with(b"*/") {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if depth != 0 {
                    return Err(LabError::Invalid("comentario SQL sin cerrar".into()));
                }
            }
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                let mut closed = false;
                while i < bytes.len() {
                    if bytes[i] == quote {
                        i += 1;
                        if bytes.get(i) == Some(&quote) {
                            i += 1;
                        } else {
                            closed = true;
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                if !closed {
                    return Err(LabError::Invalid("literal SQL sin cerrar".into()));
                }
                // Un identificador citado no se interpreta como tabla no citada.
                result.push("<quoted>".into());
            }
            b'$' => {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                if bytes.get(i) == Some(&b'$') {
                    i += 1;
                    let delimiter = &sql[start..i];
                    let end = sql[i..]
                        .find(delimiter)
                        .ok_or_else(|| LabError::Invalid("dollar quote sin cerrar".into()))?;
                    i += end + delimiter.len();
                    result.push("<body>".into());
                }
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                result.push(sql[start..i].to_ascii_lowercase());
            }
            byte if byte.is_ascii_whitespace() => i += 1,
            _ => {
                result.push(char::from(bytes[i]).to_string());
                i += 1;
            }
        }
    }
    Ok(result)
}

fn tables(sql: &str) -> Result<Vec<String>> {
    let tokens = tokens(sql)?;
    let mut tables = Vec::new();
    for (i, pair) in tokens.windows(2).enumerate() {
        if pair == ["drop", "table"] {
            return Err(LabError::Invalid(
                "DROP TABLE requiere inventario semántico adicional".into(),
            ));
        }
        if pair != ["create", "table"] {
            continue;
        }
        let name = tokens
            .get(i + 2)
            .ok_or_else(|| LabError::Invalid("CREATE TABLE incompleto".into()))?;
        if tokens.get(i + 3).map(String::as_str) != Some("(")
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(LabError::Invalid(
                "CREATE TABLE fuera del subconjunto verificable".into(),
            ));
        }
        tables.push(name.clone());
    }
    Ok(tables)
}

pub(crate) fn baseline(repo: &Path) -> Result<Value> {
    let root = repo.join("backend/src/main/resources/db/migration");
    let mut files = Vec::new();
    let mut inventory = BTreeSet::new();
    let mut versions = BTreeSet::new();
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("sql") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| LabError::Invalid("nombre SQL no UTF-8".into()))?;
        let version: u32 = name
            .strip_prefix('V')
            .and_then(|s| s.split_once("__"))
            .ok_or_else(|| LabError::Invalid(format!("nombre Flyway no soportado: {name}")))?
            .0
            .parse()
            .map_err(|_| LabError::Invalid(format!("versión Flyway inválida: {name}")))?;
        let bytes = fs::read(&path)?;
        let sql =
            std::str::from_utf8(&bytes).map_err(|_| LabError::Invalid("SQL no UTF-8".into()))?;
        let created = tables(sql)?;
        for table in &created {
            if !inventory.insert(table.clone()) {
                return Err(LabError::Invalid(format!(
                    "tabla duplicada en fuentes: {table}"
                )));
            }
        }
        if !versions.insert(version) {
            return Err(LabError::Invalid("versión Flyway duplicada".into()));
        }
        files.push(json!({"version": version, "path": format!("backend/src/main/resources/db/migration/{name}"), "sha256": sha256(&bytes), "tables_created": created}));
    }
    files.sort_by_key(|entry| entry["version"].as_u64());
    let verified = versions == (1..=25).collect() && inventory.len() == 37;
    let git_status = Command::new("git")
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ])
        .current_dir(repo)
        .output();
    let worktree = match git_status {
        Ok(output) if output.status.success() => json!({
            "status": "OBSERVED", "dirty": !output.stdout.is_empty(),
            "status_sha256": sha256(&output.stdout),
            "scope": "git status porcelain v1 -z, incluidos untracked; no es hash del contenido del worktree",
        }),
        _ => json!({"status": "NOT_RUN", "dirty": null, "reason": "git status no disponible"}),
    };
    Ok(json!({
        "status": if verified { "VERIFIED_SOURCE_INVENTORY" } else { "BLOCKED" },
        "scope": "Fuentes del checkout actual, no catálogo de una BD ejecutada ni certificación de paridad",
        "git_head": command(repo, "git", &["--no-pager", "rev-parse", "HEAD"]),
        "worktree": worktree,
        "flyway": {"expected_count": 25, "observed_count": files.len(), "files": files},
        "table_inventory": {"expected_count": 37, "observed_count": inventory.len(), "names": inventory,
            "method": "CREATE TABLE no citado, fuera de comentarios/literales/cuerpos; rechaza DROP TABLE y sintaxis no soportada; excluye flyway_schema_history"},
    }))
}

pub(crate) fn roots(repo: &Path) -> Result<Value> {
    let mut roots = Vec::new();
    for name in ["local", "central", "control"] {
        let root = repo.join("backend-rust/migrations").join(name);
        let mut files = Vec::new();
        if root.is_dir() {
            for entry in fs::read_dir(&root)? {
                let path = entry?.path();
                if path.extension().and_then(|s| s.to_str()) != Some("sql") {
                    continue;
                }
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| LabError::Invalid("nombre de migración no UTF-8".into()))?;
                let version = filename
                    .split('_')
                    .next()
                    .and_then(|s| s.parse::<i64>().ok());
                files.push(json!({"path": format!("backend-rust/migrations/{name}/{filename}"), "version": version, "sha256": sha256(fs::read(&path)?)}));
            }
        }
        files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        let digest = if files.is_empty() {
            None
        } else {
            Some(sha256(serde_json::to_vec(&files)?))
        };
        roots.push(json!({"root": name, "status": "BLOCKED", "scope": "Subconjunto F1-10; nunca esquema completo", "executed": false, "files": files, "source_inventory_sha256": digest}));
    }
    Ok(json!(roots))
}

pub(crate) fn toolchain(repo: &Path) -> Value {
    let workspace = repo.join("backend-rust");
    let metadata = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--offline", "--locked"])
        .current_dir(&workspace)
        .output();
    let packages = match metadata {
        Ok(output) if output.status.success() => {
            match serde_json::from_slice::<Value>(&output.stdout) {
                Ok(value) => {
                    json!({"status": "OBSERVED", "packages": value["packages"].as_array().map(|packages| packages.iter()
                .map(|p| json!({"name": p["name"], "version": p["version"], "source": p["source"]})).collect::<Vec<_>>())})
                }
                Err(error) => json!({"status": "NOT_RUN", "reason": error.to_string()}),
            }
        }
        _ => {
            json!({"status": "NOT_RUN", "reason": "cargo metadata --offline --locked no disponible; no se inventan versiones resueltas"})
        }
    };
    json!({
        "rustc": command(&workspace, "rustc", &["--version", "--verbose"]),
        "cargo": command(&workspace, "cargo", &["--version"]),
        "resolved_dependencies": packages,
        "cargo_lock_sha256": fs::read(workspace.join("Cargo.lock")).ok().map(sha256),
        "tool_version": env!("CARGO_PKG_VERSION"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_ignores_comments_literals_and_function_bodies() -> Result<()> {
        let sql = "-- CREATE TABLE fake (id int);\n/* CREATE TABLE fake2 (id int); */\nCREATE TABLE real_table (id integer); SELECT 'CREATE TABLE fake3 (id int)'; DO $body$ CREATE TABLE fake4(id int); $body$;";
        assert_eq!(tables(sql)?, vec!["real_table"]);
        assert!(tables("DROP TABLE real_table").is_err());
        assert!(tables("CREATE TABLE IF NOT EXISTS uncertain(id int)").is_err());
        assert!(tables("CREATE TABLE \"quoted\"(id int)").is_err());
        Ok(())
    }

    #[test]
    fn hashes_are_computed_from_bytes() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(sha256(b"abc"), sha256(b"abc\n"));
    }

    #[test]
    fn hex_preserves_leading_zeroes_and_has_fixed_lowercase_width() {
        for (input, expected) in [
            (
                b"".as_slice(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"s".as_slice(),
                "043a718774c572bd8a25adbeb1bfcd5c0256ae11cecf9f9c3f925d0e52beaf89",
            ),
        ] {
            let hash = sha256(input);
            assert_eq!(hash, expected);
            assert_eq!(hash.len(), 64);
            assert!(
                hash.bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            );
        }
    }
}
