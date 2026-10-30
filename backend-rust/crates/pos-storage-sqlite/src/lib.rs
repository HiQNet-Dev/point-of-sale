//! Adaptador SQLite de foundation; no autoriza importaciones ni escrituras de negocio.
//!
//! # Capas
//!
//! Este crate es **infraestructura**: implementa los puertos declarados en
//! [`pos_application`] y contiene todo el SQL. No define reglas de negocio ni
//! contratos de caso de uso; esos viven en `pos-core` (dominio) y
//! `pos-application` (aplicación).

/// Escritor de auditoría sobre la tabla candidata `audit_event`. No productivo.
pub mod audit;
/// Provisioning inicial sobre el esquema candidato. No productivo.
pub mod bootstrap;
/// Cajas y turnos persistentes sobre el esquema candidato. No productivo.
pub mod cash;
/// Catálogo organizacional persistente sobre el esquema candidato. No productivo.
pub mod catalog;
/// Checkout candidato: pagos, consumo y cierre de venta en una única UoW.
pub mod checkout;
/// Conexión SQLite: apertura validada, migraciones y checkpoint.
pub mod connection;
/// Transición de credenciales de laboratorio (BCrypt → Argon2id) sobre la UoW.
pub mod credential;
/// Adaptador de identidad y permisos exclusivo de laboratorio (tablas `*_lab`).
pub mod identity;
/// Inventario persistente candidato: artículos, saldos, ledger y recetas.
pub mod inventory;
mod organization_branch;
mod prelude;
/// Ventas y snapshots persistentes sobre el esquema candidato. No productivo.
pub mod sales;
mod transaction;

pub use connection::{StorageError, checkpoint, migrate, open, verify_health};
pub use organization_branch::{
    OrganizationBranchStorageError, SqliteBranchApplication, SqliteOrganizationApplication,
    SqliteOrganizationBranchStore,
};
pub use pos_application::CasOutcome;
pub use transaction::{SqliteTransaction, SqliteUnitOfWork, TransactionalAuditWriter};

// Los módulos de prueba viven en descendientes del crate y reutilizan estos
// tipos mediante `use super::*`; se exponen solo en la compilación de pruebas.
#[cfg(test)]
use sqlx::SqlitePool;

#[cfg(test)]
mod audit_writer_tests;
#[cfg(test)]
mod candidate_flow_tests;
#[cfg(test)]
mod cash_store_tests;
#[cfg(test)]
mod catalog_store_tests;
#[cfg(test)]
mod checkout_store_tests;
#[cfg(test)]
mod credential_lab_tests;
#[cfg(test)]
mod identity_lab_tests;
#[cfg(test)]
mod inventory_store_tests;
#[cfg(test)]
mod proposals_tests;
#[cfg(test)]
mod sales_store_tests;
#[cfg(test)]
mod tests;
