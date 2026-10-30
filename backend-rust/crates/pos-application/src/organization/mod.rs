//! Casos de uso de Organization y Branch sobre puertos.
//!
//! Viven en la capa de aplicación: el dominio declara las entidades y los
//! puertos; el adaptador SQLite los implementa.

mod application;
mod queries;

pub use queries::{BranchReader, list_authorized_branches};

pub use application::{BranchApplication, OrganizationApplication};
