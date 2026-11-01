//! Casos de uso de Organization y Branch.
//!
//! Viven en la capa de aplicación: el dominio declara las entidades y los
//! puertos; el adaptador SQLite los implementa.
//!
//! # Dos composiciones
//!
//! - [`administration`]: composición **productiva** y asíncrona sobre los puertos
//!   transaccionales de SQLite, con autorización desde [`pos_core::identity::VerifiedPrincipal`]
//!   y auditoría canónica USER/SYSTEM.
//! - [`OrganizationApplication`] y [`BranchApplication`]: composición síncrona
//!   sobre puertos, conservada para el laboratorio y sus pruebas. No puede
//!   sostener la frontera transaccional asíncrona ni la atribución canónica de
//!   auditoría, así que no se usa en el producto.

pub mod administration;
mod application;
mod queries;

pub use administration::{
    BranchConfiguration, BranchConfigurationUpdate, BranchCreation,
    OrganizationAdministrationApplication, OrganizationAdministrationTransaction,
    OrganizationAdministrationUnitOfWork, OrganizationConfigurationUpdate, TimezoneUpdate,
};
pub use queries::{BranchReader, list_authorized_branches};

pub use application::{BranchApplication, OrganizationApplication};
