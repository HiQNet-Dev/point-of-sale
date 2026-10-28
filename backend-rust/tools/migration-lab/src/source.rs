//! Origen Java/Flyway real, exportado y comparado sin contactar objetivos.
//!
//! El bundle contiene hashes de credenciales sintéticas: se mantiene en un
//! temporal privado y nunca se copia a la evidencia publicable.

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use pos_core::{Currency, MoneyMinor};
use serde_json::{Value, json};

use crate::{
    LabError, Result,
    report::{Case, Evidence},
};

pub(crate) struct SourceLab {
    pub report: Value,
    pub cases: Vec<Case>,
    pub counts: Value,
    pub money_vectors: Value,
    pub stock_reconciliation: Value,
}

fn repository() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| LabError::Invalid("no se pudo localizar la raíz del checkout".into()))
}

fn run_script(repo: &Path, bin: &Path, output: &Path) -> Result<()> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/java_source_lab.py");
    if !script.is_file() {
        return Err(LabError::Invalid(
            "falta scripts/java_source_lab.py para --java-source".into(),
        ));
    }
    let mut command = Command::new("python3");
    command
        .arg("-B")
        .arg(&script)
        .arg("--pg-bin")
        .arg(bin)
        .arg("--output")
        .arg(output)
        .current_dir(repo)
        .env_clear();
    for key in ["PATH", "HOME", "JAVA_HOME"] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    let result = command
        .output()
        .map_err(|_| LabError::Invalid("no se pudo iniciar el origen Java/Flyway".into()))?;
    if !result.status.success() {
        return Err(LabError::Invalid(match result.status.code() {
            Some(2) => "origen Java/Flyway incompleto (BLOCKED)".into(),
            _ => "origen Java/Flyway falló; detalles privados no publicados".into(),
        }));
    }
    Ok(())
}

fn table<'a>(bundle: &'a Value, name: &str) -> Result<&'a Value> {
    bundle["tables"]
        .as_array()
        .and_then(|tables| tables.iter().find(|table| table["name"] == name))
        .ok_or_else(|| LabError::Invalid(format!("source bundle sin tabla {name}")))
}

fn column(table: &Value, name: &str) -> Result<usize> {
    table["columns"]
        .as_array()
        .and_then(|columns| columns.iter().position(|column| column["name"] == name))
        .ok_or_else(|| LabError::Invalid(format!("source bundle sin columna {name}")))
}

fn vector(
    source: &Value,
    table_name: &str,
    column_name: &str,
    scale: u32,
    row_limit: usize,
) -> Result<Value> {
    let table = table(source, table_name)?;
    let index = column(table, column_name)?;
    let rows = table["rows"]
        .as_array()
        .ok_or_else(|| LabError::Invalid("source bundle sin filas".into()))?;
    let values = rows
        .iter()
        .take(row_limit)
        .map(|row| {
            let value = &row[index];
            if value.is_null() {
                return json!({"sql": "NULL", "result": {"status": "SQL_NULL"}});
            }
            let Some(decimal) = value.as_str() else {
                return json!({"sql": value, "result": {"status": "REJECTED", "reason": "not_decimal_text"}});
            };
            let result = Currency::new("MXN")
                .and_then(|currency| MoneyMinor::from_decimal_str(decimal, currency, scale));
            match result {
                Ok(money) => json!({
                    "sql": decimal,
                    "result": {"status": "EXACT", "minor": money.amount(), "currency": money.currency().as_str()},
                }),
                Err(error) => json!({
                    "sql": decimal,
                    "result": {"status": "REJECTED", "reason": error.to_string()},
                }),
            }
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "table": table_name,
        "column": column_name,
        "scale": scale,
        "values": values,
    }))
}

fn money_vectors(bundle: &Value) -> Result<Value> {
    // Vectores observados en el source: operandos exactos y dinero final.
    // La escala 2 de MXN hace visible que 12.3456 no se redondea en silencio.
    Ok(json!({
        "status": "TECHNICAL_OBSERVED",
        "normative_status": "BLOCKED",
        "source": "synthetic PostgreSQL 17 bundle",
        "conversion": "pos_core::MoneyMinor::from_decimal_str (no float, no rounding)",
        "vectors": [
            vector(bundle, "products", "unit_price", 4, 2)?,
            vector(bundle, "products", "unit_price", 2, 2)?,
            vector(bundle, "sales", "total_amount", 2, 1)?,
            vector(bundle, "sales", "total_amount", 4, 1)?,
            vector(bundle, "payments", "amount", 2, 1)?,
            vector(bundle, "purchase_order_lines", "unit_cost", 4, 2)?,
        ],
    }))
}

fn scaled_six(text: &str) -> Option<i128> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(value) => (true, value),
        None => (false, text),
    };
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let integer: i128 = integer.parse().ok()?;
    let fraction: i128 = if fraction.is_empty() {
        0
    } else {
        format!("{fraction:0<6}").parse().ok()?
    };
    let value = integer.checked_mul(1_000_000)?.checked_add(fraction)?;
    Some(if negative { -value } else { value })
}

fn scaled_six_text(value: i128) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.abs();
    format!("{sign}{}.{:06}", absolute / 1_000_000, absolute % 1_000_000)
}

fn stock_reconciliation(bundle: &Value) -> Result<Value> {
    let movements = table(bundle, "inventory_movements")?;
    let columns = [
        "branch_id",
        "inventory_item_id",
        "quantity_delta",
        "resulting_quantity",
        "occurred_at",
    ]
    .map(|name| column(movements, name))
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    let rows = movements["rows"]
        .as_array()
        .ok_or_else(|| LabError::Invalid("movimientos sin filas".into()))?;
    let mut ordered = (0..rows.len()).collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        rows[*left][columns[4]]
            .as_str()
            .unwrap_or("")
            .cmp(rows[*right][columns[4]].as_str().unwrap_or(""))
    });
    let mut totals: std::collections::BTreeMap<(String, String), i128> =
        std::collections::BTreeMap::new();
    let mut running: std::collections::BTreeMap<(String, String), i128> =
        std::collections::BTreeMap::new();
    let mut rows_checked = 0_u32;
    for index in ordered {
        let row = &rows[index];
        let delta = scaled_six(
            row[columns[2]]
                .as_str()
                .ok_or_else(|| LabError::Invalid("delta de inventario no textual".into()))?,
        )
        .ok_or_else(|| LabError::Invalid("delta de inventario no exacto".into()))?;
        let resulting = scaled_six(
            row[columns[3]]
                .as_str()
                .ok_or_else(|| LabError::Invalid("saldo resultante no textual".into()))?,
        )
        .ok_or_else(|| LabError::Invalid("saldo resultante no exacto".into()))?;
        let key = (
            row[columns[0]]
                .as_str()
                .ok_or_else(|| LabError::Invalid("branch sin texto".into()))?
                .to_owned(),
            row[columns[1]]
                .as_str()
                .ok_or_else(|| LabError::Invalid("item sin texto".into()))?
                .to_owned(),
        );
        let current = running.entry(key.clone()).or_insert(0);
        *current = current
            .checked_add(delta)
            .ok_or_else(|| LabError::Invalid("desbordamiento de conciliación".into()))?;
        if *current != resulting {
            return Err(LabError::Invalid(
                "resulting_quantity no coincide con el ledger acumulado".into(),
            ));
        }
        let total = totals.entry(key).or_insert(0);
        *total = total
            .checked_add(delta)
            .ok_or_else(|| LabError::Invalid("desbordamiento de conciliación".into()))?;
        rows_checked += 1;
    }
    let balances = table(bundle, "branch_inventory")?;
    let branch = column(balances, "branch_id")?;
    let item = column(balances, "inventory_item_id")?;
    let quantity = column(balances, "quantity_on_hand")?;
    let mut reconciliations = Vec::new();
    for row in balances["rows"]
        .as_array()
        .ok_or_else(|| LabError::Invalid("balances sin filas".into()))?
    {
        let key = (
            row[branch]
                .as_str()
                .ok_or_else(|| LabError::Invalid("branch sin texto".into()))?
                .to_owned(),
            row[item]
                .as_str()
                .ok_or_else(|| LabError::Invalid("item sin texto".into()))?
                .to_owned(),
        );
        let ledger = totals.get(&key).copied().unwrap_or(0);
        let balance = scaled_six(
            row[quantity]
                .as_str()
                .ok_or_else(|| LabError::Invalid("balance no textual".into()))?,
        )
        .ok_or_else(|| LabError::Invalid("balance no exacto".into()))?;
        if ledger != balance {
            return Err(LabError::Invalid(
                "branch_inventory no concilia con inventory_movements".into(),
            ));
        }
        reconciliations.push(json!({
            "branch_id": key.0,
            "inventory_item_id": key.1,
            "ledger_quantity_exact": scaled_six_text(ledger),
            "balance_quantity_exact": scaled_six_text(balance),
            "equal": true,
        }));
    }
    Ok(json!({
        "status": "TECHNICAL_OBSERVED",
        "normative_status": "BLOCKED",
        "method": "suma exacta i128 a escala 6 por branch/item; sin float",
        "movement_rows_checked": rows_checked,
        "resulting_quantity_chain_valid": true,
        "balances": reconciliations,
        "note": "No acredita AS-05: falta importación target y el dominio completo",
    }))
}

fn validate(report: &Value) -> Result<()> {
    if report["status"] != "PASSED"
        || report["normative_status"] != "UNCHANGED_BLOCKED"
        || report["target_import"] != "BLOCKED_CONFLICT_FREEZE"
    {
        return Err(LabError::Invalid(
            "source report no acredita una ejecución técnica completa".into(),
        ));
    }
    let observations = report["observations"]
        .as_array()
        .ok_or_else(|| LabError::Invalid("source report sin observations".into()))?;
    let installs = observations
        .iter()
        .filter(|observation| {
            observation["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("SOURCE-INSTALL-"))
        })
        .collect::<Vec<_>>();
    if installs.len() != 2
        || installs.iter().any(|observation| {
            observation["status"] != "PASSED"
                || observation["table_count"] != 37
                || observation["populated_tables"] != 37
        })
    {
        return Err(LabError::Invalid(
            "se requieren dos instalaciones source con 37/37 tablas".into(),
        ));
    }
    if !observations.iter().any(|observation| {
        observation["id"] == "SOURCE-RECONSTRUCTION"
            && observation["status"] == "PASSED"
            && observation["equal_functional_digests"] == true
    }) {
        return Err(LabError::Invalid(
            "las reconstrucciones source no comparten digest".into(),
        ));
    }
    Ok(())
}

/// Ejecuta el origen Java/Flyway y conserva solo artefactos publicables.
pub(crate) async fn run(bin: &Path, evidence: &Evidence) -> Result<SourceLab> {
    let repo = repository()?;
    // El CLI puede recibir una ruta relativa a backend-rust; el script se ejecuta
    // con cwd en la raíz del repositorio, así que debe ser absoluta.
    let bin = bin
        .canonicalize()
        .map_err(|_| LabError::Invalid("ruta --postgres-bin no resoluble".into()))?;
    let (report, manifest, bundle, counts) = tokio::task::spawn_blocking(move || {
        let target = repo.join("backend-rust/target");
        let private = tempfile::Builder::new()
            .prefix("java-source-lab-")
            .tempdir_in(target)?;
        let output = private.path().join("evidence");
        run_script(&repo, &bin, &output)?;
        let report: Value =
            serde_json::from_slice(&std::fs::read(output.join("source-report.json"))?)?;
        let manifest: Value =
            serde_json::from_slice(&std::fs::read(output.join("manifest.json"))?)?;
        let bundle: Value = serde_json::from_slice(&std::fs::read(output.join("bundle-1.json"))?)?;
        validate(&report)?;
        let counts = report["observations"]
            .as_array()
            .and_then(|observations| {
                observations
                    .iter()
                    .find(|item| item["id"] == "SOURCE-INSTALL-1")
            })
            .map(|observation| {
                json!({
                    "scope": "source_synthetic_only",
                    "target_import": "BLOCKED_CONFLICT_FREEZE",
                    "table_count": observation["table_count"],
                    "populated_tables": observation["populated_tables"],
                    "counts": observation["source_counts"],
                    "refresh_sessions": "INVALIDATE",
                    "app_metadata": "REFERENCE_ONLY",
                })
            })
            .ok_or_else(|| LabError::Invalid("source report sin conteos".into()))?;
        Ok::<_, LabError>((report, manifest, bundle, counts))
    })
    .await
    .map_err(|_| LabError::Invalid("worker del origen Java/Flyway falló".into()))??;

    evidence.json("source-report.json", &report)?;
    evidence.json("source-manifest.json", &manifest)?;
    let money = money_vectors(&bundle)?;
    let stock = stock_reconciliation(&bundle)?;
    let mut cases = Vec::new();
    for id in [
        "SOURCE-INSTALL-1",
        "SOURCE-INSTALL-2",
        "SOURCE-RECONSTRUCTION",
    ] {
        let observation = report["observations"]
            .as_array()
            .and_then(|observations| observations.iter().find(|item| item["id"] == id))
            .ok_or_else(|| LabError::Invalid(format!("source report sin {id}")))?;
        cases.push(Case::technical(
            id,
            if observation["status"] == "PASSED" {
                Ok(serde_json::to_string(observation)?)
            } else {
                Err(LabError::Invalid(format!("{id} no aprobado")))
            },
        ));
    }
    Ok(SourceLab {
        report,
        cases,
        counts,
        money_vectors: money,
        stock_reconciliation: stock,
    })
}
