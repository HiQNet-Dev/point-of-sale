//! Registro de señales antes de abrir recursos persistentes.

/// Conserva las señales recibidas durante la apertura y las migraciones.
pub(crate) struct ShutdownSignals {
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
}

impl ShutdownSignals {
    /// Instala los manejadores sin conservar errores del sistema operativo.
    pub(crate) fn install() -> Result<Self, ()> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let interrupt = signal(SignalKind::interrupt()).map_err(|_| ())?;
            let terminate = signal(SignalKind::terminate()).map_err(|_| ())?;
            Ok(Self {
                interrupt,
                terminate,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    /// Espera SIGINT, SIGTERM o un fallo tipado de recepción.
    #[cfg(unix)]
    pub(crate) async fn wait(mut self) -> Result<(), ()> {
        tokio::select! {
            // El stream SIGINT conserva una interrupción anterior al primer poll de ctrl_c.
            received = self.interrupt.recv() => received.ok_or(()),
            received = self.terminate.recv() => received.ok_or(()),
            result = tokio::signal::ctrl_c() => result.map_err(|_| ()),
        }
    }

    /// Espera la interrupción portable en plataformas sin señales Unix.
    #[cfg(not(unix))]
    pub(crate) async fn wait(self) -> Result<(), ()> {
        tokio::signal::ctrl_c().await.map_err(|_| ())
    }
}
