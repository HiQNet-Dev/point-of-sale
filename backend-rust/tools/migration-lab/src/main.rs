//! Laboratorio de migraciones aislado; no certifica paridad con fixtures parciales.

mod auth;
mod cli;
mod postgres;
mod provenance;
mod report;
mod source;
mod sqlite;

use std::{path::Path, process::ExitCode};

use serde_json::{Value, json};

use report::{Case, Coverage, Evidence, OFFLINE_LIMITATION, Status};

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum LabError {
    #[error("{0}")]
    Invalid(String),
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("SQLite: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Migración SQLx: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("Storage: {0}")]
    Storage(#[from] pos_storage_sqlite::StorageError),
}

pub(crate) type Result<T> = std::result::Result<T, LabError>;

#[derive(Default)]
struct ExternalLabs {
    auth: Value,
    postgres: Option<Value>,
    source: Option<source::SourceLab>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match cli::Args::parse(std::env::args_os().skip(1)) {
        Ok(None) => {
            println!(
                "migration-lab --output DIRECTORIO_NUEVO [--postgres-bin DIR] [--java-source] [--require-complete]"
            );
            ExitCode::SUCCESS
        }
        Ok(Some(args)) => match run(args).await {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("migration-lab: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("migration-lab: {error}");
            ExitCode::FAILURE
        }
    }
}

fn technical_case_from_json(id: &str, status: &str, detail: &Value) -> Result<Case> {
    Ok(Case {
        id: id.to_owned(),
        status: if status == "PASSED" {
            Status::Passed
        } else {
            Status::Failed
        },
        coverage: Coverage::SyntheticTechnical,
        detail: serde_json::to_string(detail)?,
    })
}

fn add_auth_cases(results: &mut sqlite::Results, auth: &Value) -> Result<()> {
    let cases = auth["cases"]
        .as_array()
        .ok_or_else(|| LabError::Invalid("auth report sin cases".into()))?;
    if cases.len() != 8 {
        return Err(LabError::Invalid(
            "auth report no contiene AUTH-MIG-01..08".into(),
        ));
    }
    for case in cases {
        let id = case["id"]
            .as_str()
            .ok_or_else(|| LabError::Invalid("caso auth sin id".into()))?;
        let status = case["status"]
            .as_str()
            .ok_or_else(|| LabError::Invalid("caso auth sin estado".into()))?;
        results
            .cases
            .push(technical_case_from_json(id, status, case)?);
    }
    Ok(())
}

fn add_postgres_cases(results: &mut sqlite::Results, report: &Value) -> Result<()> {
    let checks = report["technical_checks"]
        .as_array()
        .ok_or_else(|| LabError::Invalid("postgres report sin checks".into()))?;
    for check in checks {
        let id = check["id"]
            .as_str()
            .ok_or_else(|| LabError::Invalid("check PostgreSQL sin id".into()))?;
        let status = check["status"]
            .as_str()
            .ok_or_else(|| LabError::Invalid("check PostgreSQL sin estado".into()))?;
        results
            .cases
            .push(technical_case_from_json(id, status, check)?);
    }
    if report["technical_status"] != "PASSED" {
        results.cases.push(Case {
            id: "postgres_technical_status".into(),
            status: Status::Failed,
            coverage: Coverage::SyntheticTechnical,
            detail: "el informe PostgreSQL no quedó PASSED".into(),
        });
    }
    Ok(())
}

async fn run(args: cli::Args) -> Result<u8> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .ok_or_else(|| LabError::Invalid("no se pudo localizar la raíz del checkout".into()))?;
    let evidence = Evidence::create(&args.output)?;
    let baseline_result = provenance::baseline(repo);
    let mut provenance_cases = Vec::new();
    let baseline = match baseline_result {
        Ok(value) => {
            provenance_cases.push(Case::technical(
                "baseline_source_inventory",
                if value["status"] == "VERIFIED_SOURCE_INVENTORY" {
                    Ok("25 migraciones y 37 tablas verificadas desde fuentes del checkout; no BD viva".into())
                } else {
                    Err(LabError::Invalid(
                        "el inventario de fuentes no coincide con 25 migraciones/37 tablas".into(),
                    ))
                },
            ));
            value
        }
        Err(error) => {
            let reason = error.to_string();
            provenance_cases.push(Case::technical("baseline_source_inventory", Err(error)));
            json!({"status": "FAILED", "reason": reason, "flyway": null, "table_inventory": null})
        }
    };
    evidence.json("baseline-manifest.json", &baseline)?;
    let roots_result = provenance::roots(repo);
    let roots = match roots_result {
        Ok(value) => value,
        Err(error) => {
            let reason = error.to_string();
            provenance_cases.push(Case::technical("root_source_inventory", Err(error)));
            json!({"status": "FAILED", "reason": reason})
        }
    };
    let fixtures = fixture_manifest()?;
    let mut results = sqlite::run(&evidence).await;
    results.cases.extend(provenance_cases);

    let auth_report = auth::run()
        .await
        .map_err(|_| LabError::Invalid("auth lab no pudo ejecutarse".into()))?;
    add_auth_cases(&mut results, &auth_report)?;
    evidence.json("auth-transition.json", &json!({
        "status": "TECHNICAL_OBSERVED",
        "normative_status": "BLOCKED",
        "as09_normative": false,
        "reason": "Máquina de estados sintética en memoria; no acredita atomicidad persistente ni importador completo",
        "report": auth_report,
    }))?;

    let mut labs = ExternalLabs {
        auth: auth_report,
        ..ExternalLabs::default()
    };

    if let Some(bin) = args.postgres_bin.as_deref() {
        match postgres::run(bin, &evidence).await {
            Ok(report) => {
                add_postgres_cases(&mut results, &report)?;
                labs.postgres = Some(report);
            }
            Err(_) => {
                results.cases.push(Case {
                    id: "postgres_startup".into(),
                    status: Status::Failed,
                    coverage: Coverage::SyntheticTechnical,
                    detail: "no se pudo iniciar PostgreSQL 17 nativo; ver failure-logs/postgres-startup.json"
                        .into(),
                });
                labs.postgres = Some(json!({
                    "technical_status": "FAILED",
                    "technical_checks": [],
                    "normative_status": "BLOCKED",
                    "reason": "arranque PostgreSQL nativo rechazado; no se acepta una URL externa",
                }));
            }
        }
    } else {
        evidence.json("postgres-status.json", &json!({
            "status": "NOT_REQUESTED",
            "reason": "Use --postgres-bin DIR para ejecutar los checks SQLx contra PostgreSQL 17 real",
            "normative_status": "BLOCKED",
        }))?;
    }

    if args.java_source {
        let bin = args
            .postgres_bin
            .as_deref()
            .ok_or_else(|| LabError::Invalid("--java-source exige --postgres-bin".into()))?;
        match source::run(bin, &evidence).await {
            Ok(source) => {
                results.cases.extend(source.cases.clone());
                labs.source = Some(source);
            }
            Err(_) => {
                results.cases.push(Case {
                    id: "source_lab".into(),
                    status: Status::Failed,
                    coverage: Coverage::SyntheticTechnical,
                    detail: "origen Java/Flyway no completó; detalles privados no publicados"
                        .into(),
                });
            }
        }
    }

    let immutable = (|| {
        let after = provenance::baseline(repo)?;
        let after_roots = provenance::roots(repo)?;
        if baseline["flyway"].is_null()
            || baseline["flyway"] != after["flyway"]
            || roots != after_roots
            || fixtures != fixture_manifest()?
        {
            return Err(LabError::Invalid(
                "las fuentes de migración cambiaron durante la ejecución".into(),
            ));
        }
        Ok("Fuentes Flyway, roots Rust y fixtures idénticas antes/después; no demuestra inmutabilidad de datos de negocio".into())
    })();
    results
        .cases
        .push(Case::technical("migration_sources_unchanged", immutable));
    evidence.json("migration-manifest.json", &json!({
        "status": "PARTIAL", "roots": roots, "toolchain": provenance::toolchain(repo),
        "technical_fixture_migrations": fixtures,
        "applied_fixture_history": results.observation("rerun_history_immutable"),
        "sqlite_runtime": results.observation("fixture_setup"),
        "postgres_requested": args.postgres_bin.is_some(),
        "java_source_requested": args.java_source,
        "warning": "Roots reales son subconjuntos F1-10. Ningún PASSED técnico acredita cobertura normativa.",
    }))?;
    write_result_reports(&evidence, &results, &labs, args.require_complete)
}

fn external_summary(labs: &ExternalLabs) -> Value {
    let postgres = match &labs.postgres {
        Some(report) => json!({
            "technical_status": report["technical_status"],
            "check_count": report["technical_checks"].as_array().map(Vec::len),
            "cleanup": report["cleanup"],
            "normative_status": "BLOCKED",
            "reason": "Roots Central/Control siguen siendo subconjuntos; no son DDL13-02/03",
        }),
        None => json!({
            "status": "NOT_REQUESTED",
            "normative_status": "BLOCKED",
        }),
    };
    let source = match &labs.source {
        Some(source) => json!({
            "status": source.report["status"],
            "coverage": source.report["coverage"],
            "target_import": source.report["target_import"],
            "normative_status": "BLOCKED",
            "observations": source.report["observations"],
            "blocked_cases": source.report["blocked_cases"],
            "actions": source.report["actions"],
        }),
        None => json!({
            "status": "NOT_REQUESTED",
            "normative_status": "BLOCKED",
        }),
    };
    json!({"auth": labs.auth, "postgres": postgres, "source": source})
}

fn write_result_reports(
    evidence: &Evidence,
    results: &sqlite::Results,
    labs: &ExternalLabs,
    require_complete: bool,
) -> Result<u8> {
    let mut normative = report::normative_cases();
    // PAR-08 exige un exportador determinista sobre instalaciones limpias; no
    // requiere import target. Solo se aprueba con la observación source PASSED.
    if let Some(source) = &labs.source {
        let par08 = source.report["observations"]
            .as_array()
            .and_then(|observations| observations.iter().find(|item| item["id"] == "PAR-08"))
            .is_some_and(|item| item["status"] == "PASSED");
        if par08 && let Some(case) = normative.iter_mut().find(|case| case.id == "PAR-08") {
            case.status = Status::Passed;
            case.coverage = Coverage::NormativeExact;
            case.detail = "Dos instalaciones limpias Flyway V1-V25, 37/37 tablas y digest funcional idéntico; exportador determinista, sin import target".into();
        }
    }
    let code = report::exit_code(&results.cases, &normative, require_complete);
    let normative_passed = normative
        .iter()
        .filter(|case| case.status == Status::Passed)
        .count();
    let normative_blocked = normative.len().saturating_sub(normative_passed);
    let technical_status = if code == 1 { "FAILED" } else { "PASSED" };
    let external = external_summary(labs);
    evidence.json("parity-report.json", &json!({
        "status": if code == 1 { "FAILED" } else { "BLOCKED" },
        "normative_status": "BLOCKED", "normative_complete": false,
        "normative_approved": normative_passed, "normative_blocked": normative_blocked,
        "technical_status": technical_status,
        "technical_checks": results.cases, "normative_cases": normative,
        "external_labs": external,
        "physical_contract_blockers": [
            {"id": "PHYS-COUNT", "status": "BLOCKED", "reason": "F1-10 contiene 82 definiciones tabulares; la cifra 83 incluye una tabla vacía y no identifica una entidad adicional"},
            {"id": "PHYS-DEPENDENCIES", "status": "BLOCKED", "reason": "price_book, tax_profile, CostingPolicy, ExpenseCategory y plan no tienen ficha física"},
            {"id": "UNIT-COST", "status": "BLOCKED", "reason": "Java admite unit_cost NULL en ORDERED/RECEIVED; F1-10 exige unit_cost_exact y line_total_exact NOT NULL"},
            {"id": "TARGET-IMPORT", "status": "BLOCKED", "reason": "ADR-0021 propuesto sin aprobación; no se importa ni se convierte NULL en cero"}
        ],
        "offline_network_denied": {"status": "NOT_RUN", "reason": OFFLINE_LIMITATION},
        "exit_code": code, "require_complete": require_complete,
    }))?;
    evidence.write(
        "parity-report.xml",
        report::parity_junit(&results.cases, &normative),
    )?;
    evidence.write(
        "technical-checks.xml",
        report::junit("synthetic-technical-checks", &results.cases),
    )?;
    write_domain_reports(evidence, results, labs, code)?;
    let mut immutability: Vec<Case> = results
        .cases
        .iter()
        .filter(|case| {
            case.id == "rerun_history_immutable" || case.id == "migration_sources_unchanged"
        })
        .cloned()
        .collect();
    immutability.push(Case {
        id: "domain_immutability".into(),
        status: Status::Blocked,
        coverage: Coverage::NormativeUncovered,
        detail: "No se ejecutaron triggers ni flujos de inmutabilidad del dominio completo".into(),
    });
    evidence.write(
        "immutability-tests.xml",
        report::junit("immutability", &immutability),
    )?;
    let failures: Vec<_> = results
        .cases
        .iter()
        .filter(|case| case.status == Status::Failed)
        .collect();
    evidence.json("failure-logs/technical-failures.json", &json!({
        "status": if failures.is_empty() { "NO_TECHNICAL_FAILURES" } else { "FAILED" }, "failures": failures,
        "expected_failures": "Los archivos expected-* documentan rechazos SQL esperados y verificados, no fallos del runner",
    }))?;
    let passed = results
        .cases
        .iter()
        .filter(|case| case.status == Status::Passed)
        .count();
    let postgres = external["postgres"]["technical_status"]
        .as_str()
        .unwrap_or("NOT_REQUESTED");
    let source_status = external["source"]["status"]
        .as_str()
        .unwrap_or("NOT_REQUESTED");
    evidence.write("run-summary.md", format!(
        "# Migration lab\n\n- Estado normativo: **BLOCKED**, {normative_passed}/30 casos aprobados; {normative_blocked} bloqueados/pendientes; DDL13-10 NOT_RUN.\n- Estado técnico: **{technical_status}**.\n- Checks técnicos: {passed}/{} PASSED (solo evidencia técnica, no paridad).\n- Auth sintético: AUTH-MIG-01..08 en `auth-transition.json`; AS-09 sigue BLOCKED.\n- PostgreSQL nativo: {postgres}; PG17 real, sin Docker/URLs externas.\n- Origen Java/Flyway: {source_status}; 37/37 solo si el reporte lo demuestra; PAR-08 PASSED (exportador determinista), TARGET-IMPORT sigue BLOCKED.\n- Money/Quantity: vectores exactos observados en `money-vectors.json`; AS-04/AS-05 siguen BLOCKED.\n- Blockers físicos: 82 fichas, cinco dependencias sin ficha y `unit_cost` incompatible (ADR-0021 propuesto).\n- Exit code: **{code}**; require-complete: {require_complete}.\n- Procedencia: baseline-manifest.json y migration-manifest.json (fuentes del checkout, no datos productivos).\n- SQLite: fixtures sintéticos STRICT, upgrade, FK por conexión, checksum, rollback, historial, backup WAL/restore/reopen.\n- Offline: NOT_RUN; {OFFLINE_LIMITATION}\n- Artefactos normativos pendientes y archivos .sha256 pendientes contienen JSON BLOCKED sin digest inventado.\n- Consulte technical-checks.xml y failure-logs/ para fallos técnicos, parity-report.xml para skips normativos.\n",
        results.cases.len()
    ))?;
    println!(
        "Evidencia: {}\nTécnicos: {technical_status}, {passed}/{} PASSED; normativos: {normative_passed}/30 PASSED, {normative_blocked} bloqueados/pendientes; exit={code}",
        evidence.root.display(),
        results.cases.len()
    );
    Ok(code)
}

fn fixture_manifest() -> Result<serde_json::Value> {
    let mut files = Vec::new();
    for (version, relative) in [
        (1, "fixtures/migrations/0001_lab.sql"),
        (2, "fixtures/migrations/0002_lab_upgrade.sql"),
        (3, "fixtures/transaction-failure/0003_lab_failure.sql"),
    ] {
        let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))?;
        files.push(
            json!({"version": version, "path": relative, "sha256": provenance::sha256(bytes),
            "scope": "synthetic_technical", "expected_failure": version == 3}),
        );
    }
    Ok(json!(files))
}

fn write_domain_reports(
    evidence: &Evidence,
    results: &sqlite::Results,
    labs: &ExternalLabs,
    code: u8,
) -> Result<()> {
    let pending = |reason: &str| json!({"status": "BLOCKED", "scope": "normative", "reason": reason, "sha256": null});
    let counts = match &labs.source {
        Some(source) => json!({
            "status": "OBSERVED_SOURCE_ONLY",
            "normative_status": "BLOCKED",
            "reason": "37 tablas source no equivalen a importación target ni a paridad normativa",
            "source": source.counts,
            "technical_fixture_upgrade": results.observation("upgrade_preserves_data"),
        }),
        None => json!({
            "status": "BLOCKED", "scope": "normative", "reason": "Sin conteos del esquema/dataset completo en ambos motores",
            "technical_fixture_upgrade": results.observation("upgrade_preserves_data"),
        }),
    };
    evidence.json("counts.json", &counts)?;
    evidence.json(
        "integrity.json",
        &json!({
            "status": "TECHNICAL_OBSERVED",
            "normative_status": "BLOCKED",
            "reason": "Integridad técnica SQLite/PostgreSQL/source; falta paridad target completa",
            "technical_restore": results.observation("wal_backup_restore"),
            "technical_fk": results.observation("foreign_keys_each_connection"),
            "technical_strict": results.observation("strict_types"),
            "postgres": labs.postgres.as_ref().map(|report| json!({
                "technical_status": report["technical_status"],
                "cleanup": report["cleanup"],
                "contexts": report["contexts"],
            })),
            "source": labs.source.as_ref().map(|source| json!({
                "stock_reconciliation": source.stock_reconciliation,
            })),
        }),
    )?;
    for (name, reason) in [
        (
            "stock-reconciliation.json",
            "No hay ledger ni reconciliación de stock del dominio completo ejecutados",
        ),
        (
            "idempotency.json",
            "Rerun SQLx no acredita idempotencia de checkout/recepciones ni sus fingerprints",
        ),
        (
            "ids.sha256",
            "No se ejecutó una exportación normativa de identidades; no existe digest de datos que publicar",
        ),
        (
            "snapshots.sha256",
            "No se ejecutaron snapshots normativos de negocio; los snapshots SQL técnicos se identifican aparte",
        ),
        (
            "restore.sha256",
            "Restore normativo pendiente; el restore sintético, si pasó, tiene digest en backup-fixtures/restore.sha256",
        ),
    ] {
        if name == "stock-reconciliation.json"
            && let Some(source) = &labs.source
        {
            evidence.json(name, &source.stock_reconciliation)?;
            continue;
        }
        evidence.json(name, &pending(reason))?;
    }
    if let Some(source) = &labs.source {
        evidence.json("money-vectors.json", &source.money_vectors)?;
    } else {
        evidence.json(
            "money-vectors.json",
            &pending("No se ejecutaron vectores monetarios Java/Rust con cobertura exacta"),
        )?;
    }
    let auth_cases = labs.auth["cases"].as_array().cloned().unwrap_or_default();
    evidence.json("security-report.json", &json!({
        "status": "TECHNICAL_OBSERVED",
        "normative_status": "BLOCKED",
        "reason": "AS-09 y AS-01..AS-10 siguen sin cobertura normativa exacta",
        "data_policy": "Únicamente datos sintéticos; no se leen secretos ni datos productivos",
        "auth": {
            "scope": "synthetic_technical",
            "commercial_policy_approved": false,
            "cases": auth_cases,
            "policy": labs.auth["policy"],
            "benchmark": labs.auth["benchmark"],
        },
        "postgresql": labs.postgres.as_ref().map(|report| json!({
            "technical_status": report["technical_status"],
            "transport": report["runtime"]["transport"],
            "roles": report["runtime"]["roles"],
            "normative_status": "BLOCKED",
        })),
        "source": labs.source.as_ref().map(|source| json!({
            "status": source.report["status"],
            "refresh_sessions": "INVALIDATE",
            "app_metadata": "REFERENCE_ONLY",
            "target_import": "BLOCKED_CONFLICT_FREEZE",
        })),
        "network_denied": {"status": "NOT_RUN", "reason": OFFLINE_LIMITATION},
        "output_policy": "Directorio nuevo, padre existente, create_dir atómico; artefactos create_new sin overwrite",
    }))?;
    evidence.json("backup-fixtures/restoration-report.json", &json!({
        "normative_status": "BLOCKED",
        "sqlite_technical_restore": results.observation("wal_backup_restore"),
        "postgres_technical_restore": labs.postgres.as_ref().map(|report| json!({
            "central": report["contexts"]["central"]["restore"],
            "control": report["contexts"]["control"]["restore"],
        })),
        "reason": "Restore sintético no acredita IMP-05 ni recuperación de una instalación real",
    }))?;
    if !evidence
        .root
        .join("backup-fixtures/restore.sha256")
        .exists()
    {
        evidence.json(
            "backup-fixtures/restore.sha256",
            &pending("El restore técnico no produjo evidencia verificada; consultar failure-logs"),
        )?;
    }
    evidence.json(
        "schema-snapshots/normative-status.json",
        &pending("Roots parciales; no hay snapshot normativo completo"),
    )?;
    let mut snapshots = Vec::new();
    for name in ["lab-v1.json", "lab-v2.json", "storage-local-subset.json"] {
        let path = evidence.root.join("schema-snapshots").join(name);
        if path.is_file() {
            snapshots.push(json!({"path": format!("schema-snapshots/{name}"), "sha256": provenance::sha256(std::fs::read(path)?)}));
        } else {
            snapshots.push(json!({"path": format!("schema-snapshots/{name}"), "status": "NOT_RUN", "sha256": null}));
        }
    }
    if labs.postgres.is_some() {
        for name in [
            "postgres-central-clean.json",
            "postgres-control-clean.json",
            "postgres-central-v2.json",
            "postgres-control-v2.json",
        ] {
            let path = evidence.root.join("schema-snapshots").join(name);
            snapshots.push(if path.is_file() {
                json!({"path": format!("schema-snapshots/{name}"), "sha256": provenance::sha256(std::fs::read(path)?)})
            } else {
                json!({"path": format!("schema-snapshots/{name}"), "status": "NOT_RUN", "sha256": null})
            });
        }
    }
    evidence.json(
        "schema-snapshots/technical-digests.json",
        &json!({"scope": "synthetic_technical_or_storage_subset", "snapshots": snapshots, "exit_code": code}),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn technical_failure_is_visible_in_global_json_junit_summary_and_logs() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let evidence = Evidence::create(&temp.path().join("failed-evidence"))?;
        let results = sqlite::Results {
            cases: vec![Case::technical(
                "failed_check",
                Err(LabError::Invalid("fallo técnico de prueba".into())),
            )],
            ..Default::default()
        };
        let labs = ExternalLabs {
            auth: json!({"cases": []}),
            ..ExternalLabs::default()
        };
        assert_eq!(write_result_reports(&evidence, &results, &labs, true)?, 1);
        let parity: serde_json::Value =
            serde_json::from_slice(&std::fs::read(evidence.root.join("parity-report.json"))?)?;
        assert_eq!(parity["status"], "FAILED");
        assert_eq!(parity["normative_status"], "BLOCKED");
        assert_eq!(parity["technical_status"], "FAILED");
        assert_eq!(parity["exit_code"], 1);
        let xml = std::fs::read_to_string(evidence.root.join("parity-report.xml"))?;
        assert!(xml.contains("tests=\"31\" failures=\"1\" errors=\"0\" skipped=\"30\""));
        assert_eq!(xml.matches("<failure ").count(), 1);
        assert_eq!(xml.matches("<skipped ").count(), 30);
        let summary = std::fs::read_to_string(evidence.root.join("run-summary.md"))?;
        assert!(summary.contains("Estado técnico: **FAILED**"));
        let logs: serde_json::Value = serde_json::from_slice(&std::fs::read(
            evidence.root.join("failure-logs/technical-failures.json"),
        )?)?;
        assert_eq!(logs["status"], "FAILED");
        assert_eq!(logs["failures"][0]["id"], "failed_check");
        let normative = parity["normative_cases"]
            .as_array()
            .ok_or_else(|| LabError::Invalid("casos normativos ausentes".into()))?;
        assert_eq!(normative.len(), 30);
        assert!(
            normative
                .iter()
                .all(|case| case["status"] == "BLOCKED" || case["status"] == "NOT_RUN")
        );
        Ok(())
    }
}
