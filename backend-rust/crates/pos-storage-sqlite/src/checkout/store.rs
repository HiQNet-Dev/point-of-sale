//! Checkout candidato sobre SQLite real: pagos, consumo y cierre de venta en una
//! única unidad de trabajo.
//!
//! **No productivo.** Las tablas `payment` y `sale_item_inventory_component`
//! viven en `proposals/local-sales/0015` y se aplican solo en pruebas. No existe
//! ruta de aplicación que marque una venta `COMPLETED` sin pagos, sin resolución
//! de recetas y sin movimientos de ledger en la misma transacción.
//!
//! # Orden de la transacción
//!
//! 1. Autorización `sale.checkout` y scope derivado del principal.
//! 2. Lectura de la venta con CAS de revisión y exigencia de `OPEN`.
//! 3. Recalculo exacto del total desde las líneas activas.
//! 4. Validación de la composición de pagos antes de escribir nada.
//! 5. Inserción de pagos y snapshots de consumo por línea.
//! 6. Verificación de existencias según la política de la organización.
//! 7. Movimientos de ledger y saldos por CAS.
//! 8. Transición a `COMPLETED` con CAS y auditoría obligatoria.
//! 9. Commit único; cualquier fallo revierte todo el conjunto.

use pos_core::organization_branch::ApplicationError;

use crate::SqliteUnitOfWork;

pub(crate) fn map_checkout_write(error: &sqlx::Error) -> ApplicationError {
    let Some(database) = error.as_database_error() else {
        return ApplicationError::Storage;
    };
    if database.is_foreign_key_violation() {
        ApplicationError::OwnershipMismatch
    } else if database.is_check_violation() || database.is_unique_violation() {
        ApplicationError::InvalidSaleInput
    } else {
        ApplicationError::Storage
    }
}

pub use pos_application::checkout::{
    CheckoutApplication, CheckoutOutcome, CheckoutPaymentRequest, CheckoutRequest,
    CheckoutTransaction,
};

/// Alias de compatibilidad: el caso de uso vive ahora en `pos-application`.
pub type SqliteCheckoutApplication = CheckoutApplication<SqliteUnitOfWork>;
