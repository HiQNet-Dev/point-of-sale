//! DTOs, puertos y casos de uso de inventario.

mod dto;
mod ports;
mod use_cases;

pub use dto::*;
pub use ports::{InventoryTransaction, InventoryUnitOfWork};
pub use use_cases::InventoryApplication;
