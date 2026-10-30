//! Soporte explícitamente sintético para integración; no se enlaza desde pos-local.
//! Las propuestas se ejecutan solo en SQLite temporal del laboratorio.

mod contract;
mod operations;
mod scenario;

pub use contract::{LabErrorCode, ScenarioReport, bindings};
pub use operations::catalog as operation_catalog;
pub use scenario::{authenticate as authenticate_candidate_session, run, verify_reopen};
