//! Puertos de la administración del negocio.
//!
//! Declarados en la capa que los consume. El adaptador SQLite los implementa
//! sobre la misma frontera transaccional que el resto de módulos: una conexión
//! con `BEGIN IMMEDIATE`, la mutación, la auditoría y un único commit.

use std::future::Future;

use pos_core::organization_branch::{ApplicationError, Branch, EntityId, Organization};

use crate::{CasOutcome, audit::AuditAppender};

/// Operaciones transaccionales de configuración del negocio.
///
/// Toda lectura y escritura de sucursal exige `organization_id`: una fila de
/// otra organización se comporta como inexistente y la clave foránea compuesta
/// del esquema lo respalda.
pub trait OrganizationAdministrationTransaction: AuditAppender {
    /// Organización por identidad.
    fn get_organization(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Organization>, ApplicationError>> + Send;
    /// Aplica CAS sobre la revisión previa.
    fn update_organization_cas(
        &mut self,
        previous_revision: u64,
        organization: &Organization,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Sucursal scoped por organización.
    fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Branch>, ApplicationError>> + Send;
    /// Sucursales de la organización, ordenadas por código.
    fn list_branches(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<Branch>, ApplicationError>> + Send;
    /// Número de sucursales de la organización, para la política de capacidad.
    fn count_branches(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<u64, ApplicationError>> + Send;
    /// Inserta una sucursal nueva.
    fn insert_branch(
        &mut self,
        branch: &Branch,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Comprueba si el código de sucursal ya existe en la organización.
    fn branch_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &pos_core::organization_branch::BusinessCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Aplica CAS scoped por organización y revisión.
    fn update_branch_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        branch: &Branch,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
}

/// Unidad de trabajo de la administración del negocio.
pub trait OrganizationAdministrationUnitOfWork {
    /// Transacción que comparte mutación, CAS y auditoría.
    type Transaction: OrganizationAdministrationTransaction + Send;

    /// Abre la transacción de escritura.
    fn begin_transaction(
        &self,
    ) -> impl Future<Output = Result<Self::Transaction, ApplicationError>> + Send;
    /// Confirma una operación completa.
    fn commit_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Revierte una operación fallida.
    fn rollback_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}
