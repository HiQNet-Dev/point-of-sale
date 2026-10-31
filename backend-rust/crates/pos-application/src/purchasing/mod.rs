//! Casos de uso, DTOs y puertos de compras.
//!
//! Compras reutiliza el dominio de `pos-core` (`supplier`, `purchasing`) y la
//! misma unidad de trabajo transaccional que caja y ventas. La aplicación decide
//! autorización, CAS y auditoría; el adaptador aporta SQL y persistencia.
//!
//! # Garantías
//!
//! - El actor y la organización proceden del principal verificado.
//! - Toda mutación auditable comparte una única transacción física.
//! - El costo pactado desconocido es `None` y nunca se convierte en cero.
//! - Una recepción mueve stock, escribe ledger y audita en el mismo commit.

mod dto;
mod ports;
mod use_cases;

pub use dto::*;
pub use ports::{PurchasingTransaction, PurchasingUnitOfWork};
pub use use_cases::{MAX_PURCHASING_PAGE_SIZE, PurchasingApplication, supplier_total};
