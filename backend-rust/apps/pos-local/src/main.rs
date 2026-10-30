//! Composición explícita del proceso local: configuración, SQLite y cierre ordenado.

mod signals;
mod telemetry;

use pos_application::installation::{ApprovedSchema, InstallationReport, prepare_installation};
use pos_local::{config::Config, shutdown::checkpoint_and_close};
use pos_storage_sqlite::{
    approved_schema_version, checkpoint, installation::SqliteInstallationSchema, open,
    verify_health,
};
use signals::ShutdownSignals;
use std::process::ExitCode;
use telemetry::{Event, emit};

/// Comprueba el estado del proceso para el healthcheck del contenedor.
///
/// Verifica que el archivo exista, que SQLite lo abra y que el migrador tenga
/// migraciones aplicadas. No crea el archivo: un volumen vacío es un fallo.
async fn health() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            emit(Event::ConfigurationRejected(error));
            return ExitCode::FAILURE;
        }
    };
    if !config.db_path().exists() {
        eprintln!("{{\"event\":\"health_database_missing\"}}");
        return ExitCode::FAILURE;
    }
    let pool = match open(config.db_path(), config.busy_timeout()).await {
        Ok(pool) => pool,
        Err(_) => {
            emit(Event::StorageOpenFailed);
            return ExitCode::FAILURE;
        }
    };
    let result = verify_health(&pool).await;
    pool.close().await;
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("{{\"event\":\"health_check_failed\"}}");
            ExitCode::FAILURE
        }
    }
}

/// Prepara la instalación traduciendo el fallo a un evento propio.
async fn prepare(
    schema: &SqliteInstallationSchema,
    approved: ApprovedSchema,
) -> Result<InstallationReport, Event> {
    prepare_installation(schema, schema, approved)
        .await
        .map_err(Event::InstallationRejected)
}

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().any(|argument| argument == "--health") {
        return health().await;
    }
    if telemetry::init().is_err() {
        // No imprimir el error del suscriptor, que podría incorporar datos externos.
        eprintln!("{{\"event\":\"telemetry_initialization_failed\"}}");
        return ExitCode::FAILURE;
    }
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            emit(Event::ConfigurationRejected(error));
            return ExitCode::FAILURE;
        }
    };
    let signals = match ShutdownSignals::install() {
        Ok(signals) => signals,
        Err(()) => {
            emit(Event::SignalSetupFailed);
            return ExitCode::FAILURE;
        }
    };
    let existed = config.db_path().exists();
    let pool = match open(config.db_path(), config.busy_timeout()).await {
        Ok(pool) => pool,
        Err(_) => {
            emit(Event::StorageOpenFailed);
            return ExitCode::FAILURE;
        }
    };
    let schema = SqliteInstallationSchema::new(pool.clone(), existed);
    let approved = match ApprovedSchema::new(approved_schema_version()) {
        Ok(approved) => approved,
        // Deriva del migrador embebido: un valor inválido es un defecto de build.
        Err(error) => {
            emit(Event::InstallationRejected(error));
            pool.close().await;
            return ExitCode::FAILURE;
        }
    };

    // Después de open no hay retornos tempranos que omitan la liberación del pool.
    let operation = async {
        // Clasifica antes de migrar para distinguir primera instalación de reapertura
        // y rechazar bases creadas por un binario más reciente.
        let report = prepare(&schema, approved).await?;
        emit(Event::InstallationResolved(report));
        emit(Event::Ready);
        signals.wait().await.map_err(|()| Event::SignalWaitFailed)?;
        emit(Event::ShutdownRequested);
        Ok::<(), Event>(())
    }
    .await;

    let operation_failed = match operation {
        Ok(()) => false,
        Err(event) => {
            // Registrar el fallo inicial antes del checkpoint, que también puede fallar.
            emit(event);
            true
        }
    };
    // SQLx 0.9 marca el pool como cerrado al invocar close(), no al esperar su futuro.
    // Diferir la invocación mantiene el pool disponible hasta terminar el checkpoint.
    let cleanup = checkpoint_and_close(checkpoint(&pool), async { pool.close().await }).await;
    if cleanup.is_err() {
        emit(Event::CheckpointFailed);
    }
    if operation_failed || cleanup.is_err() {
        ExitCode::FAILURE
    } else {
        emit(Event::Stopped);
        ExitCode::SUCCESS
    }
}
