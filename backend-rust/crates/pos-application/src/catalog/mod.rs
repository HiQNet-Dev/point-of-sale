//! DTOs, puertos y casos de uso del catálogo.

mod dto;
mod ports;
mod use_cases;

pub use dto::*;
pub use ports::{CatalogTransaction, CatalogUnitOfWork};
pub use use_cases::CatalogApplication;
