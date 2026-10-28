//! Telemetría JSON de vocabulario cerrado, sin datos operativos sensibles.

use pos_local::config::ConfigError;
use tracing::Subscriber;
use tracing_subscriber::{
    Layer, filter::filter_fn, fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt,
};

const TARGET: &str = "pos_local::lifecycle";

/// Eventos propios; ninguna variante almacena rutas, SQL ni errores originales.
pub(crate) enum Event {
    ConfigurationRejected(ConfigError),
    SignalSetupFailed,
    StorageOpenFailed,
    MigrationFailed,
    Ready,
    ShutdownRequested,
    SignalWaitFailed,
    CheckpointFailed,
    Stopped,
}

/// Instala un suscriptor sin filtros controlados por variables de entorno.
pub(crate) fn init() -> Result<(), ()> {
    subscriber(std::io::stderr).try_init().map_err(|_| ())
}

fn subscriber<W>(writer: W) -> impl Subscriber + Send + Sync
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(writer)
            .with_target(false)
            .with_file(false)
            .with_line_number(false)
            .with_current_span(false)
            .with_span_list(false)
            // Excluir también ERROR/WARN de dependencias: podrían contener SQL o rutas.
            .with_filter(filter_fn(|metadata| metadata.target() == TARGET)),
    )
}

/// Emite únicamente identificadores estáticos y motivos de configuración tipados.
pub(crate) fn emit(event: Event) {
    match event {
        Event::Ready => tracing::info!(target: TARGET, event = "ready"),
        Event::ShutdownRequested => tracing::info!(target: TARGET, event = "shutdown_requested"),
        Event::Stopped => tracing::info!(target: TARGET, event = "stopped"),
        Event::ConfigurationRejected(reason) => {
            tracing::error!(target: TARGET, event = "configuration_rejected", reason = reason.code());
        }
        Event::SignalSetupFailed => tracing::error!(target: TARGET, event = "signal_setup_failed"),
        Event::StorageOpenFailed => tracing::error!(target: TARGET, event = "storage_open_failed"),
        Event::MigrationFailed => tracing::error!(target: TARGET, event = "migration_failed"),
        Event::SignalWaitFailed => tracing::error!(target: TARGET, event = "signal_wait_failed"),
        Event::CheckpointFailed => tracing::error!(target: TARGET, event = "checkpoint_failed"),
    }
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
