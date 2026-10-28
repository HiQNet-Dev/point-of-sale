//! Secuencia de liberación que conserva el fallo del checkpoint.

use std::future::Future;

/// Espera el checkpoint y luego el cierre, incluso si el checkpoint devuelve error.
///
/// No devuelve el resultado hasta completar el cierre. El llamador debe esperar
/// este futuro hasta el final: no protege frente a cancelación, pánico o SIGKILL.
///
/// # Errors
/// Propaga el error original del checkpoint después de completar el cierre.
pub async fn checkpoint_and_close<E>(
    checkpoint: impl Future<Output = Result<(), E>>,
    close: impl Future<Output = ()>,
) -> Result<(), E> {
    let result = checkpoint.await;
    close.await;
    result
}

#[cfg(test)]
#[path = "shutdown_tests.rs"]
mod tests;
