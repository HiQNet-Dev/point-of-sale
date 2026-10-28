//! Composición explícita del proceso local: configuración, SQLite y cierre ordenado.

mod signals;
mod telemetry;

use pos_local::{config::Config, shutdown::checkpoint_and_close};
use pos_storage_sqlite::{checkpoint, migrate, open};
use signals::ShutdownSignals;
use std::process::ExitCode;
use telemetry::{Event, emit};

#[tokio::main]
async fn main() -> ExitCode {
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
    let pool = match open(config.db_path(), config.busy_timeout()).await {
        Ok(pool) => pool,
        Err(_) => {
            emit(Event::StorageOpenFailed);
            return ExitCode::FAILURE;
        }
    };

    // Después de open no hay retornos tempranos que omitan la liberación del pool.
    let operation = async {
        migrate(&pool).await.map_err(|_| Event::MigrationFailed)?;
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
