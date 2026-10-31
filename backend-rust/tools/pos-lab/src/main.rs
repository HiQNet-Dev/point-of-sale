//! CLI aislado; nunca utiliza una base productiva.
use pos_lab::{LabErrorCode, ScenarioReport, bindings, operation_catalog, run, verify_reopen};
use pos_storage_sqlite::open;
use serde_json::json;
use std::{error::Error, path::PathBuf, time::Duration};

fn database_path() -> PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--database"
            && let Some(value) = args.next()
        {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("pos-lab.sqlite")
}

#[tokio::main]
async fn main() {
    if execute().await.is_err() {
        // La frontera pública no devuelve errores SQL ni material de identidad.
        eprintln!("{}", json!({ "code": LabErrorCode::LabScenarioFailed }));
        std::process::exit(1);
    }
}

async fn execute() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|argument| argument == "--export-operations") {
        println!("{}", serde_json::to_string_pretty(&operation_catalog()?)?);
        return Ok(());
    }
    if std::env::args().any(|argument| argument == "--export-contract") {
        print!("{}", bindings());
        return Ok(());
    }
    let contract_report = std::env::args().any(|argument| argument == "--contract-report");
    let path = database_path();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    // Reserva exclusiva: nunca borrar ni reutilizar una base suministrada por el usuario.
    let reservation = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    drop(reservation);
    let pool = open(&path, Duration::from_millis(500)).await?;
    let flow = run(&pool).await;
    let reopened = match &flow {
        Ok(report) => {
            pool.close().await;
            verify_reopen(&path, report).await
        }
        Err(_) => {
            pool.close().await;
            Err("flujo no completado".into())
        }
    };

    let report = match (flow, reopened) {
        (Ok(mut flow), Ok(reopen)) => {
            flow["reopen"] = reopen;
            flow
        }
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };

    let typed = ScenarioReport::from_internal(&report)?;
    if contract_report {
        println!("{}", serde_json::to_string(&typed)?);
    } else {
        println!("{report}");
    }
    Ok(())
}
