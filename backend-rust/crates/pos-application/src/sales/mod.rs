//! DTOs, puertos y casos de uso de ventas.

mod dto;
mod ports;
mod use_cases;

pub use dto::*;
pub use ports::{SalesTransaction, SalesUnitOfWork};
pub use use_cases::{SalesApplication, sale_error};
