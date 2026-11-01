//! Adaptador SQLite de la administración del negocio.
//!
//! Implementa los puertos de `pos-application::organization::administration`
//! sobre la misma frontera transaccional que el resto de módulos: una conexión
//! con `BEGIN IMMEDIATE`, la mutación, el CAS, la escritura de auditoría y un
//! único commit. Si la auditoría falla, la mutación se revierte con ella.
//!
//! # Ownership
//!
//! Todas las operaciones de sucursal incluyen `organization_id` en el `WHERE`.
//! Una fila de otra organización se comporta como inexistente, y la clave foránea
//! compuesta `(organization_id, id)` del esquema respalda la barrera.

use pos_application::organization::administration::{
    OrganizationAdministrationApplication, OrganizationAdministrationTransaction,
    OrganizationAdministrationUnitOfWork,
};
use pos_core::organization_branch::{
    ApplicationError, Branch, BusinessCode, EntityId, Organization,
};

use crate::prelude::transaction_error;
use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork};

/// Alias de composición: los casos de uso viven en `pos-application`.
pub type SqliteOrganizationAdministration = OrganizationAdministrationApplication<SqliteUnitOfWork>;

impl OrganizationAdministrationUnitOfWork for SqliteUnitOfWork {
    type Transaction = SqliteTransaction;

    async fn begin_transaction(&self) -> Result<SqliteTransaction, ApplicationError> {
        SqliteUnitOfWork::begin(self)
            .await
            .map_err(transaction_error)
    }

    async fn commit_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.commit().await.map_err(transaction_error)
    }

    async fn rollback_transaction(
        &self,
        transaction: SqliteTransaction,
    ) -> Result<(), ApplicationError> {
        transaction.rollback().await.map_err(transaction_error)
    }
}

impl OrganizationAdministrationTransaction for SqliteTransaction {
    async fn get_organization(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Option<Organization>, ApplicationError> {
        SqliteTransaction::get_organization(self, organization_id)
            .await
            .map_err(transaction_error)
    }

    async fn update_organization_cas(
        &mut self,
        previous_revision: u64,
        organization: &Organization,
        occurred_at: &str,
    ) -> Result<CasOutcome, ApplicationError> {
        SqliteTransaction::update_organization_cas(
            self,
            previous_revision,
            organization,
            occurred_at,
        )
        .await
        .map_err(transaction_error)
    }

    async fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Option<Branch>, ApplicationError> {
        SqliteTransaction::get_branch(self, organization_id, branch_id)
            .await
            .map_err(transaction_error)
    }

    async fn list_branches(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Vec<Branch>, ApplicationError> {
        let rows = sqlx::query_as::<_, crate::organization_branch::BranchRow>(
            "SELECT id, organization_id, code, name, status, timezone_override, version \
             FROM branch WHERE organization_id = ? ORDER BY code ASC",
        )
        .bind(organization_id.as_str())
        .fetch_all(self.connection())
        .await
        .map_err(|_| ApplicationError::Storage)?;
        rows.into_iter()
            .map(|row| row.into_domain().map_err(|_| ApplicationError::Storage))
            .collect()
    }

    async fn count_branches(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<u64, ApplicationError> {
        SqliteTransaction::count_branches(self, organization_id)
            .await
            .map_err(transaction_error)
    }

    async fn insert_branch(
        &mut self,
        branch: &Branch,
        occurred_at: &str,
    ) -> Result<(), ApplicationError> {
        SqliteTransaction::insert_branch(self, branch, occurred_at)
            .await
            .map_err(transaction_error)
    }

    async fn branch_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &BusinessCode,
    ) -> Result<bool, ApplicationError> {
        SqliteTransaction::branch_code_exists(self, organization_id, code)
            .await
            .map_err(transaction_error)
    }

    async fn update_branch_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        branch: &Branch,
        occurred_at: &str,
    ) -> Result<CasOutcome, ApplicationError> {
        SqliteTransaction::update_branch_cas(
            self,
            organization_id,
            previous_revision,
            branch,
            occurred_at,
        )
        .await
        .map_err(transaction_error)
    }
}
