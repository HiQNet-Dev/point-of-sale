//! Pruebas técnicas reales sobre SQLite y datos sintéticos, separadas de la paridad.

use std::{collections::BTreeMap, fs, path::Path, time::Duration};

use serde_json::{Value, json};
use sqlx::{Row, SqlitePool, migrate::Migrator};

use crate::{
    LabError, Result,
    provenance::sha256,
    report::{Case, Coverage, Evidence, Status},
};

const BASE: &str = "0001_lab.sql";
const UPGRADE: &str = "0002_lab_upgrade.sql";
const CHECKS: [&str; 9] = [
    "fixture_setup",
    "foreign_keys_each_connection",
    "strict_types",
    "upgrade_preserves_data",
    "checksum_mismatch",
    "transactional_failure",
    "rerun_history_immutable",
    "wal_backup_restore",
    "storage_contract_subset",
];

#[derive(Default)]
pub(crate) struct Results {
    pub cases: Vec<Case>,
    pub observations: BTreeMap<String, Value>,
}

impl Results {
    fn record(&mut self, id: &str, result: Result<Value>) {
        let result = match result {
            Ok(value) => {
                let detail = serde_json::to_string(&value).map_err(LabError::from);
                self.observations.insert(id.into(), value);
                detail
            }
            Err(error) => Err(error),
        };
        self.cases.push(Case::technical(id, result));
    }

    pub fn observation(&self, id: &str) -> Value {
        self.observations.get(id).cloned().unwrap_or_else(|| json!({
            "status": self.cases.iter().find(|case| case.id == id).map(|case| case.status).unwrap_or(Status::NotRun),
            "reason": "Sin evidencia producida; consultar technical_checks y failure-logs",
        }))
    }
}

fn require(condition: bool, reason: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(LabError::Invalid(reason.into()))
    }
}

fn fixture_root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/migrations"))
}

fn copy_fixtures(destination: &Path, upgrade: bool) -> Result<()> {
    fs::copy(fixture_root().join(BASE), destination.join(BASE))?;
    if upgrade {
        fs::copy(fixture_root().join(UPGRADE), destination.join(UPGRADE))?;
    }
    Ok(())
}

async fn seed_v1(pool: &SqlitePool) -> Result<()> {
    let v1 = tempfile::tempdir()?;
    copy_fixtures(v1.path(), false)?;
    Migrator::new(v1.path()).await?.run(pool).await?;
    sqlx::query(
        "INSERT INTO lab_parent(id, label) VALUES (1, 'synthetic-parent'), (2, 'synthetic-second')",
    )
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO lab_child(id, parent_id, quantity) VALUES (11, 1, 7), (12, 2, 0)")
        .execute(pool)
        .await?;
    Ok(())
}

/// Mantiene adquiridas todas las conexiones para comprobar cada conexión física del pool.
async fn foreign_keys(pool: &SqlitePool) -> Result<Value> {
    let count = pool.options().get_max_connections();
    require(
        count > 0 && count <= 64,
        "tamaño del pool fuera del límite seguro del laboratorio",
    )?;
    let mut connections = Vec::new();
    for _ in 0..count {
        connections.push(pool.acquire().await?);
    }
    for (i, connection) in connections.iter_mut().enumerate() {
        let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut **connection)
            .await?;
        require(
            enabled == 1,
            "foreign_keys no está activado en cada conexión",
        )?;
        let index =
            i64::try_from(i).map_err(|_| LabError::Invalid("índice fuera de rango".into()))?;
        let result =
            sqlx::query("INSERT INTO lab_child(id, parent_id, quantity) VALUES (?, 999999, 1)")
                .bind(-100 - index)
                .execute(&mut **connection)
                .await;
        match result {
            Err(error)
                if error
                    .as_database_error()
                    .and_then(|db| db.code())
                    .as_deref()
                    == Some("787") => {}
            Err(error) => return Err(error.into()),
            Ok(_) => return Err(LabError::Invalid("se aceptó una FK huérfana".into())),
        }
    }
    Ok(
        json!({"status": "PASSED", "scope": "synthetic_technical", "physical_connections_checked": connections.len(), "orphan_insert_rejected_each_connection": true}),
    )
}

async fn strict_types(pool: &SqlitePool) -> Result<Value> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_list WHERE name IN ('lab_parent', 'lab_child') AND strict = 1 ORDER BY name")
        .fetch_all(pool).await?;
    require(
        tables.len() == 2,
        "ambas tablas del fixture deben ser STRICT",
    )?;
    let result = sqlx::query(
        "INSERT INTO lab_child(id, parent_id, quantity) VALUES (98, 1, 'not-an-integer')",
    )
    .execute(pool)
    .await;
    match result {
        Err(error)
            if error
                .as_database_error()
                .and_then(|db| db.code())
                .as_deref()
                == Some("3091") => {}
        Err(error) => return Err(error.into()),
        Ok(_) => return Err(LabError::Invalid("STRICT aceptó texto en INTEGER".into())),
    }
    Ok(
        json!({"status": "PASSED", "scope": "synthetic_technical", "strict_tables": tables, "datatype_constraint_rejected": true}),
    )
}

async fn original_data(pool: &SqlitePool) -> Result<Value> {
    let parents = sqlx::query("SELECT id, label FROM lab_parent ORDER BY id")
        .fetch_all(pool)
        .await?;
    let children = sqlx::query("SELECT id, parent_id, quantity FROM lab_child ORDER BY id")
        .fetch_all(pool)
        .await?;
    let parents: Vec<Value> = parents
        .iter()
        .map(|row| -> Result<Value> {
            Ok(json!([
                row.try_get::<i64, _>("id")?,
                row.try_get::<String, _>("label")?
            ]))
        })
        .collect::<Result<_>>()?;
    let children: Vec<Value> = children
        .iter()
        .map(|row| -> Result<Value> {
            Ok(json!([
                row.try_get::<i64, _>("id")?,
                row.try_get::<i64, _>("parent_id")?,
                row.try_get::<i64, _>("quantity")?
            ]))
        })
        .collect::<Result<_>>()?;
    Ok(json!({"parents": parents, "children": children}))
}

async fn all_data(pool: &SqlitePool) -> Result<Value> {
    let data = original_data(pool).await?;
    let rows = sqlx::query("SELECT id, note FROM lab_parent ORDER BY id")
        .fetch_all(pool)
        .await?;
    let notes: Vec<Value> = rows
        .iter()
        .map(|row| -> Result<Value> {
            Ok(json!([
                row.try_get::<i64, _>("id")?,
                row.try_get::<String, _>("note")?
            ]))
        })
        .collect::<Result<_>>()?;
    Ok(json!({"original": data, "notes": notes}))
}

async fn schema(pool: &SqlitePool) -> Result<Value> {
    let rows = sqlx::query("SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type, name")
        .fetch_all(pool).await?;
    let entries: Vec<Value> = rows.iter().map(|row| -> Result<Value> {
        Ok(json!({"type": row.try_get::<String, _>("type")?, "name": row.try_get::<String, _>("name")?,
            "table": row.try_get::<String, _>("tbl_name")?, "sql": row.try_get::<String, _>("sql")?}))
    }).collect::<Result<_>>()?;
    Ok(json!(entries))
}

/// Incluye installed_on, execution_time y checksum, no solamente el número de filas.
async fn history(pool: &SqlitePool) -> Result<Value> {
    let rows = sqlx::query("SELECT version, description, CAST(installed_on AS TEXT) AS installed_on, success, hex(checksum) AS checksum, execution_time FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool).await?;
    let entries: Vec<Value> = rows.iter().map(|row| -> Result<Value> {
        Ok(json!({"version": row.try_get::<i64, _>("version")?, "description": row.try_get::<String, _>("description")?,
            "installed_on": row.try_get::<String, _>("installed_on")?, "success": row.try_get::<bool, _>("success")?,
            "checksum": row.try_get::<String, _>("checksum")?, "execution_time": row.try_get::<i64, _>("execution_time")?}))
    }).collect::<Result<_>>()?;
    Ok(json!(entries))
}

async fn upgrade(pool: &SqlitePool, evidence: &Evidence) -> Result<Value> {
    let before = original_data(pool).await?;
    let before_schema = schema(pool).await?;
    evidence.json(
        "schema-snapshots/lab-v1.json",
        &json!({"status": "OBSERVED", "scope": "synthetic_technical", "schema": before_schema}),
    )?;
    Migrator::new(fixture_root()).await?.run(pool).await?;
    let after = original_data(pool).await?;
    require(
        before == after,
        "upgrade modificó datos o identidades existentes",
    )?;
    let notes: Vec<String> = sqlx::query_scalar("SELECT note FROM lab_parent ORDER BY id")
        .fetch_all(pool)
        .await?;
    require(
        notes == ["upgraded", "upgraded"],
        "columna añadida sin valores esperados",
    )?;
    let indexes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_schema WHERE type='index' AND name='lab_child_parent_idx'",
    )
    .fetch_one(pool)
    .await?;
    require(indexes == 1, "el índice del upgrade no existe")?;
    let after_schema = schema(pool).await?;
    evidence.json(
        "schema-snapshots/lab-v2.json",
        &json!({"status": "OBSERVED", "scope": "synthetic_technical", "schema": after_schema}),
    )?;
    Ok(
        json!({"status": "PASSED", "scope": "synthetic_technical", "before_sha256": sha256(serde_json::to_vec(&before)?),
        "after_sha256": sha256(serde_json::to_vec(&after)?), "parent_count": 2, "child_count": 2, "new_column_verified": true, "new_index_verified": true}),
    )
}

async fn checksum_mismatch(pool: &SqlitePool, evidence: &Evidence) -> Result<Value> {
    let changed = tempfile::tempdir()?;
    copy_fixtures(changed.path(), true)?;
    let mut sql = fs::read(changed.path().join(BASE))?;
    sql.extend_from_slice(b"\n-- checksum mutation, synthetic test only\n");
    fs::write(changed.path().join(BASE), sql)?;
    let before = history(pool).await?;
    let before_schema = schema(pool).await?;
    let result = Migrator::new(changed.path()).await?.run(pool).await;
    let error = match result {
        Err(error @ sqlx::migrate::MigrateError::VersionMismatch(1)) => error,
        Err(error) => return Err(error.into()),
        Ok(()) => {
            return Err(LabError::Invalid(
                "SQLx aceptó una migración aplicada alterada".into(),
            ));
        }
    };
    require(
        before == history(pool).await? && before_schema == schema(pool).await?,
        "el checksum mismatch modificó historia/esquema",
    )?;
    evidence.write(
        "failure-logs/expected-checksum-mismatch.txt",
        format!("Fallo esperado y verificado: {error}\n"),
    )?;
    Ok(
        json!({"status": "PASSED", "scope": "synthetic_technical", "rejected_version": 1, "history_unchanged": true, "schema_unchanged": true}),
    )
}

async fn transactional_failure(pool: &SqlitePool, evidence: &Evidence) -> Result<Value> {
    let failing = tempfile::tempdir()?;
    copy_fixtures(failing.path(), true)?;
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/transaction-failure/0003_lab_failure.sql"),
        failing.path().join("0003_lab_failure.sql"),
    )?;
    let before = history(pool).await?;
    let data = all_data(pool).await?;
    let result = Migrator::new(failing.path()).await?.run(pool).await;
    let error = match result {
        Err(error) if error.to_string().contains("lab_missing") => error,
        Err(error) => return Err(error.into()),
        Ok(()) => return Err(LabError::Invalid("la migración inválida no falló".into())),
    };
    let partial: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name='lab_partial'")
            .fetch_one(pool)
            .await?;
    require(partial == 0, "quedó DDL parcialmente aplicado")?;
    require(
        before == history(pool).await? && data == all_data(pool).await?,
        "el rollback dejó historia o datos parciales",
    )?;
    evidence.write(
        "failure-logs/expected-transactional-failure.txt",
        format!("Fallo esperado y rollback verificado: {error}\n"),
    )?;
    Ok(
        json!({"status": "PASSED", "scope": "synthetic_technical", "partial_table_absent": true, "history_unchanged": true, "data_unchanged": true}),
    )
}

async fn rerun(pool: &SqlitePool) -> Result<Value> {
    let before = history(pool).await?;
    let data_before = all_data(pool).await?;
    let schema_before = schema(pool).await?;
    let migrator = Migrator::new(fixture_root()).await?;
    migrator.run(pool).await?;
    migrator.run(pool).await?;
    let after = history(pool).await?;
    require(before == after, "rerun modificó el historial SQLx")?;
    require(
        data_before == all_data(pool).await? && schema_before == schema(pool).await?,
        "rerun modificó esquema/datos",
    )?;
    require(
        after.as_array().is_some_and(|rows| rows.len() == 2),
        "se esperaban dos migraciones aplicadas",
    )?;
    Ok(
        json!({"status": "PASSED", "scope": "synthetic_technical", "reruns": 2,
        "before_history_sha256": sha256(serde_json::to_vec(&before)?), "after_history_sha256": sha256(serde_json::to_vec(&after)?),
        "history": after, "data_unchanged": true, "schema_unchanged": true}),
    )
}

async fn integrity(pool: &SqlitePool) -> Result<Value> {
    let integrity: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(pool)
        .await?;
    let fk = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await?;
    require(
        integrity == ["ok"] && fk.is_empty(),
        "falló integrity_check/foreign_key_check",
    )?;
    Ok(json!({"integrity_check": integrity, "foreign_key_violations": fk.len()}))
}

async fn backup_restore(pool: &SqlitePool, db: &Path, evidence: &Evidence) -> Result<Value> {
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(pool)
        .await?;
    require(
        mode.eq_ignore_ascii_case("wal"),
        "se requiere WAL real para el backup",
    )?;
    let mut held = pool.acquire().await?;
    sqlx::query("PRAGMA wal_autocheckpoint=0")
        .execute(&mut *held)
        .await?;
    sqlx::query(
        "INSERT INTO lab_parent(id, label, note) VALUES (3, 'synthetic-wal', 'committed-in-wal')",
    )
    .execute(&mut *held)
    .await?;
    let wal = db.with_extension("sqlite-wal");
    let wal_bytes = fs::metadata(&wal)?.len();
    require(wal_bytes > 32, "no hay frames WAL reales antes del backup")?;
    drop(held);
    let before = all_data(pool).await?;
    let schema_before = schema(pool).await?;
    let history_before = history(pool).await?;
    let backup = evidence.root.join("backup-fixtures/lab-backup.sqlite");
    let backup_name = backup
        .to_str()
        .ok_or_else(|| LabError::Invalid("VACUUM INTO requiere ruta UTF-8".into()))?;
    // Un parámetro SQL evita interpolar la ruta proporcionada por el usuario.
    sqlx::query("VACUUM INTO ?")
        .bind(backup_name)
        .execute(pool)
        .await?;
    let restored_path = evidence.root.join("backup-fixtures/lab-restored.sqlite");
    let bytes = fs::read(&backup)?;
    evidence.write("backup-fixtures/lab-restored.sqlite", &bytes)?;
    let restored = pos_storage_sqlite::open(&restored_path, Duration::from_secs(5)).await?;
    let validation = async {
        let restored_data = all_data(&restored).await?;
        require(
            before == restored_data,
            "restore no conserva el digest de datos",
        )?;
        require(
            schema_before == schema(&restored).await?
                && history_before == history(&restored).await?,
            "restore alteró esquema/historia",
        )?;
        let checks = integrity(&restored).await?;
        Ok::<_, LabError>((restored_data, checks))
    }
    .await;
    restored.close().await;
    let (restored_data, checks) = validation?;
    let reopened = pos_storage_sqlite::open(&restored_path, Duration::from_secs(5)).await?;
    let reopen_check = async {
        require(
            before == all_data(&reopened).await?,
            "reopen perdió datos restaurados",
        )?;
        integrity(&reopened).await
    }
    .await;
    reopened.close().await;
    reopen_check?;
    pos_storage_sqlite::checkpoint(pool).await?;
    let restored_file_sha256 = sha256(fs::read(&restored_path)?);
    evidence.write(
        "backup-fixtures/restore.sha256",
        format!("{restored_file_sha256}  lab-restored.sqlite\n"),
    )?;
    Ok(
        json!({"status": "PASSED", "scope": "synthetic_technical", "method": "VACUUM INTO, copia del backup cerrado, restore y reopen",
        "journal_mode": mode, "wal_bytes_before_backup": wal_bytes, "source_data_sha256": sha256(serde_json::to_vec(&before)?),
        "restored_data_sha256": sha256(serde_json::to_vec(&restored_data)?), "backup_file_sha256": sha256(bytes),
        "restored_file_sha256": restored_file_sha256, "reopened": true, "integrity": checks,
        "schema_and_history_equal": true, "scope_warning": "Solo lab_parent/lab_child sintéticos; no restore normativo del dominio"}),
    )
}

async fn storage_contract(directory: &Path, evidence: &Evidence) -> Result<Value> {
    let pool = pos_storage_sqlite::open(
        &directory.join("storage-subset.sqlite"),
        Duration::from_secs(5),
    )
    .await?;
    let result = async {
        pos_storage_sqlite::migrate(&pool).await?;
        let before = history(&pool).await?;
        require(before.as_array().is_some_and(|rows| !rows.is_empty()), "storage migrate no aplicó migraciones reales")?;
        pos_storage_sqlite::migrate(&pool).await?;
        require(before == history(&pool).await?, "storage migrate alteró el historial en rerun")?;
        pos_storage_sqlite::checkpoint(&pool).await?;
        let snapshot = schema(&pool).await?;
        evidence.json("schema-snapshots/storage-local-subset.json", &json!({"status": "OBSERVED", "scope": "F1-10 subset, not complete", "schema": snapshot}))?;
        Ok(json!({"status": "PASSED", "scope": "storage_api_technical_subset_only", "migrate_rerun_history_unchanged": true,
            "integrity": integrity(&pool).await?, "warning": "No acredita ningún caso DDL13/PAR/AS"}))
    }.await;
    pool.close().await;
    result
}

pub(crate) async fn run(evidence: &Evidence) -> Results {
    let mut results = Results::default();
    let run = async {
        let temp = tempfile::tempdir()?;
        let db = temp.path().join("lab.sqlite");
        let pool = pos_storage_sqlite::open(&db, Duration::from_secs(5)).await?;
        let seeded = seed_v1(&pool).await;
        if let Err(error) = seeded {
            pool.close().await;
            return Err(error);
        }
        let version: String = sqlx::query_scalar("SELECT sqlite_version()").fetch_one(&pool).await?;
        results.record("fixture_setup", Ok(json!({"status": "PASSED", "scope": "synthetic_technical", "sqlite_version": version, "seeded_parents": 2, "seeded_children": 2})));
        results.record("foreign_keys_each_connection", foreign_keys(&pool).await);
        results.record("strict_types", strict_types(&pool).await);
        results.record("upgrade_preserves_data", upgrade(&pool, evidence).await);
        results.record("checksum_mismatch", checksum_mismatch(&pool, evidence).await);
        results.record("transactional_failure", transactional_failure(&pool, evidence).await);
        results.record("rerun_history_immutable", rerun(&pool).await);
        results.record("wal_backup_restore", backup_restore(&pool, &db, evidence).await);
        pool.close().await;
        results.record("storage_contract_subset", storage_contract(temp.path(), evidence).await);
        Ok::<_, LabError>(())
    }.await;
    if let Err(error) = run {
        results.record("fixture_setup", Err(error));
    }
    for id in CHECKS {
        if !results.cases.iter().any(|case| case.id == id) {
            results.cases.push(Case {
                id: id.into(),
                status: Status::NotRun,
                coverage: Coverage::SyntheticTechnical,
                detail: "Prerrequisito técnico fallido; prueba no ejecutada".into(),
            });
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_sqlite_checks_without_promoting_partial_fixtures() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let evidence = Evidence::create(&temp.path().join("evidence"))?;
        let results = run(&evidence).await;
        assert_eq!(results.cases.len(), CHECKS.len());
        for case in &results.cases {
            assert_eq!(case.status, Status::Passed, "{}: {}", case.id, case.detail);
        }
        assert_eq!(
            crate::report::exit_code(&results.cases, &crate::report::normative_cases(), true),
            2
        );
        let restore = results.observation("wal_backup_restore");
        assert_eq!(
            restore["source_data_sha256"],
            restore["restored_data_sha256"]
        );
        assert_eq!(restore["reopened"], true);
        assert!(
            evidence
                .root
                .join("backup-fixtures/restore.sha256")
                .is_file()
        );
        Ok(())
    }
}
