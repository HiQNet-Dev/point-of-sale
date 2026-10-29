//! Prueba del ejecutable y de sus evidencias sin servicios externos ni datos reales.

use std::{error::Error, fs, path::Path, process::Command};

use serde_json::Value;

fn run(output: &Path, complete: bool) -> std::io::Result<std::process::Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_migration-lab"));
    command.arg("--output").arg(output);
    if complete {
        command.arg("--require-complete");
    }
    command.output()
}

#[test]
fn executable_emits_honest_reports_and_refuses_existing_output() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    // Las comillas comprueban que VACUUM INTO recibe la ruta como parámetro SQL.
    let normal = temp.path().join("normal ' evidence");
    let output = run(&normal, false)?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parity: Value = serde_json::from_slice(&fs::read(normal.join("parity-report.json"))?)?;
    assert_eq!(parity["status"], "BLOCKED");
    assert_eq!(parity["technical_status"], "PASSED");
    assert_eq!(parity["normative_complete"], false);
    let normative = parity["normative_cases"]
        .as_array()
        .ok_or("normative_cases ausente")?;
    assert_eq!(normative.len(), 30);
    assert!(
        normative
            .iter()
            .all(|case| case["status"] == "BLOCKED" || case["status"] == "NOT_RUN")
    );
    assert_eq!(
        normative
            .iter()
            .find(|case| case["id"] == "DDL13-10")
            .ok_or("DDL13-10 ausente")?["status"],
        "NOT_RUN"
    );
    let offline_detail = normative
        .iter()
        .find(|case| case["id"] == "DDL13-10")
        .and_then(|case| case["detail"].as_str())
        .ok_or("detalle DDL13-10 ausente")?;
    assert!(offline_detail.contains("no certifica"));
    assert!(offline_detail.contains("esquema completo"));
    assert!(offline_detail.contains("wrapper externo"));
    assert_eq!(parity["offline_network_denied"]["status"], "NOT_RUN");
    assert_eq!(parity["offline_network_denied"]["reason"], offline_detail);
    for case in normative {
        let id = case["id"].as_str().ok_or("id normativo ausente")?;
        assert!(id.starts_with("DDL13-") || id.starts_with("PAR-") || id.starts_with("AS-"));
    }
    let security: Value = serde_json::from_slice(&fs::read(normal.join("security-report.json"))?)?;
    assert_eq!(security["network_denied"]["status"], "NOT_RUN");
    assert_eq!(security["network_denied"]["reason"], offline_detail);
    let summary = fs::read_to_string(normal.join("run-summary.md"))?;
    assert!(summary.contains(offline_detail));
    let xml = fs::read_to_string(normal.join("parity-report.xml"))?;
    assert_eq!(xml.matches("<skipped ").count(), 30);
    let technical = parity["technical_checks"]
        .as_array()
        .ok_or("technical_checks ausente")?;
    assert!(technical.iter().all(|case| case["status"] == "PASSED"));
    assert_eq!(xml.matches("<testcase ").count(), technical.len() + 30);
    assert_eq!(xml.matches("<system-out>").count(), technical.len());
    assert!(!xml.contains("<failure "));
    for case in xml.split("<testcase ").skip(1) {
        if case.starts_with("classname=\"NormativeUncovered\"") {
            let body = case
                .split("</testcase>")
                .next()
                .ok_or("testcase sin cierre")?;
            assert!(body.contains("<skipped "));
            assert!(!body.contains("<system-out>"));
        }
    }
    let baseline: Value =
        serde_json::from_slice(&fs::read(normal.join("baseline-manifest.json"))?)?;
    assert_eq!(baseline["flyway"]["observed_count"], 25);
    assert_eq!(baseline["table_inventory"]["observed_count"], 37);
    let run_result: Value = serde_json::from_slice(&fs::read(normal.join("run-result.json"))?)?;
    assert_eq!(run_result["technical_status"], "PASSED");
    assert_eq!(run_result["normative_status"], "BLOCKED");
    assert_eq!(run_result["require_complete"], false);
    assert_eq!(run_result["exit_code"], 0);
    assert_eq!(
        run_result["baseline_evidence"]["path"],
        "baseline-manifest.json"
    );
    assert!(run_result["baseline_evidence"]["sha256"].is_string());
    let aggregate: Value =
        serde_json::from_slice(&fs::read(normal.join("aggregate-status.json"))?)?;
    assert_eq!(aggregate["status"], "BLOCKED");
    assert_eq!(aggregate["technical_status"], "PASSED");
    assert_eq!(aggregate["normative_status"], "BLOCKED");
    assert_eq!(
        aggregate["baseline_evidence_status"],
        "VERIFIED_SOURCE_INVENTORY"
    );
    assert_eq!(aggregate["gate"]["status"], "NOT_REQUESTED");
    assert_eq!(aggregate["gate"]["exit_code"], 0);
    for path in [
        "migration-manifest.json",
        "run-result.json",
        "aggregate-status.json",
        "counts.json",
        "integrity.json",
        "money-vectors.json",
        "stock-reconciliation.json",
        "idempotency.json",
        "auth-transition.json",
        "security-report.json",
        "run-summary.md",
        "schema-snapshots/lab-v1.json",
        "schema-snapshots/lab-v2.json",
        "schema-snapshots/storage-local-subset.json",
        "schema-snapshots/technical-digests.json",
        "backup-fixtures/restoration-report.json",
        "backup-fixtures/lab-backup.sqlite",
        "backup-fixtures/lab-restored.sqlite",
        "backup-fixtures/restore.sha256",
        "failure-logs/expected-checksum-mismatch.txt",
        "failure-logs/expected-transactional-failure.txt",
        "failure-logs/technical-failures.json",
        "immutability-tests.xml",
        "technical-checks.xml",
    ] {
        assert!(normal.join(path).is_file(), "artefacto ausente: {path}");
    }
    for name in ["ids.sha256", "snapshots.sha256", "restore.sha256"] {
        let pending: Value = serde_json::from_slice(&fs::read(normal.join(name))?)?;
        assert_eq!(pending["status"], "BLOCKED");
        assert!(pending["sha256"].is_null());
    }
    let before = fs::read(normal.join("parity-report.json"))?;
    assert_eq!(run(&normal, false)?.status.code(), Some(1));
    assert_eq!(before, fs::read(normal.join("parity-report.json"))?);
    let strict = temp.path().join("strict");
    let strict_output = run(&strict, true)?;
    assert_eq!(
        strict_output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&strict_output.stderr)
    );
    let strict_report: Value =
        serde_json::from_slice(&fs::read(strict.join("parity-report.json"))?)?;
    assert_eq!(strict_report["technical_status"], "PASSED");
    assert_eq!(strict_report["exit_code"], 2);
    let strict_run: Value = serde_json::from_slice(&fs::read(strict.join("run-result.json"))?)?;
    let strict_aggregate: Value =
        serde_json::from_slice(&fs::read(strict.join("aggregate-status.json"))?)?;
    assert_eq!(strict_run["require_complete"], true);
    assert_eq!(strict_run["exit_code"], 2);
    assert_eq!(strict_aggregate["status"], "BLOCKED");
    assert_eq!(strict_aggregate["gate"]["status"], "BLOCKED");
    assert_eq!(strict_aggregate["gate"]["exit_code"], 2);
    Ok(())
}

#[test]
fn invalid_arguments_and_help_do_not_create_outputs() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let output = temp.path().join("not-created");
    let result = Command::new(env!("CARGO_BIN_EXE_migration-lab"))
        .args(["--invalid", "--output"])
        .arg(&output)
        .output()?;
    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
    assert!(
        Command::new(env!("CARGO_BIN_EXE_migration-lab"))
            .arg("--help")
            .output()?
            .status
            .success()
    );
    Ok(())
}
