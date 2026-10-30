//! Tipos compartidos por los adaptadores SQLite.
//!
//! Uso interno del crate: evita repetir la misma lista de imports en `store`,
//! `mapper` y `repository` de cada contexto.

pub(crate) use pos_core::{
    audit::*, cash::*, catalog::*, identity::*, inventory::*, organization_branch::*, recipe::*,
    sales::*,
};

pub(crate) use crate::{CasOutcome, SqliteTransaction};

/// Traduce un error de la UoW de organización/sucursal a error de aplicación.
///
/// Todas las transacciones de contexto comparten el mismo traductor: los errores
/// de `BEGIN`/`COMMIT`/`ROLLBACK` nunca se filtran como errores de negocio del
/// contexto que los invoca.
pub(crate) fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}
