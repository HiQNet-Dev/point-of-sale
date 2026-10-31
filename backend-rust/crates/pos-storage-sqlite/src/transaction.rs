//! Unidad de trabajo y transacción SQLite del backend Local.
//!
//! `BEGIN IMMEDIATE` reserva el escritor antes de la primera lectura, de modo
//! que una mutación auditable y su evento de auditoría ven el mismo estado y no
//! pueden intercalarse con otra escritura. `sqlx::Transaction` aporta rollback
//! RAII si el futuro se cancela o el valor se abandona sin confirmar.

use pos_core::organization_branch::{ApplicationError, AuditEvent, Branch, EntityId, Organization};
use sqlx::{SqlStr, Sqlite, SqliteConnection, SqlitePool, Transaction};

use crate::organization_branch::{
    BranchRow, OrganizationBranchStorageError, OrganizationRow, map_branch_write_error,
    map_organization_write_error, revision_to_i64,
};

/// Unidad de trabajo SQLx experimental: una sola conexión SQLite por mutación.
#[derive(Clone)]
pub struct SqliteUnitOfWork {
    pool: SqlitePool,
}

impl SqliteUnitOfWork {
    /// Crea una UoW sobre el pool Local ya configurado.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Abre una transacción de escritura con reserva inmediata de SQLite.
    ///
    /// `BEGIN IMMEDIATE` se entrega a la construcción nativa de [`Transaction`],
    /// por lo que conserva su rollback RAII si el futuro se cancela o el valor se
    /// abandona antes de confirmar. La sentencia es estática; no se construye a
    /// partir de datos de entrada.
    pub async fn begin(&self) -> Result<SqliteTransaction, OrganizationBranchStorageError> {
        let connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        let transaction =
            Transaction::begin(connection, Some(SqlStr::from_static("BEGIN IMMEDIATE")))
                .await
                .map_err(|_| OrganizationBranchStorageError::Database)?;
        Ok(SqliteTransaction { transaction })
    }

    /// Lee sin abrir una transacción de escritura; las lecturas no deben tomar la
    /// reserva inmediata destinada a mutaciones auditables.
    pub async fn get_organization(
        &self,
        organization_id: &EntityId,
    ) -> Result<Option<Organization>, OrganizationBranchStorageError> {
        crate::organization_branch::SqliteOrganizationBranchStore::new(self.pool.clone())
            .get_organization(organization_id)
            .await
    }
}

/// Transacción Local con ownership exclusivo de la conexión SQLx.
///
/// No clona conexiones ni usa `block_on`. `sqlx::Transaction` hace rollback al
/// abandonarse sin commit; los métodos nunca usan el pool mientras esté activa.
pub struct SqliteTransaction {
    pub(crate) transaction: Transaction<'static, Sqlite>,
}

impl SqliteTransaction {
    /// Busca una organización mediante la conexión transaccional.
    pub async fn get_organization(
        &mut self,
        organization_id: &EntityId,
    ) -> Result<Option<Organization>, OrganizationBranchStorageError> {
        let row = sqlx::query_as::<_, OrganizationRow>("SELECT id, code, name, currency, timezone, locale, stock_negative_policy, status, version FROM organization WHERE id = ?")
            .bind(organization_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        row.map(OrganizationRow::into_domain).transpose()
    }

    /// Busca una sucursal exclusivamente dentro de su organización.
    pub async fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Option<Branch>, OrganizationBranchStorageError> {
        let row = sqlx::query_as::<_, BranchRow>("SELECT id, organization_id, code, name, status, timezone_override, version FROM branch WHERE organization_id = ? AND id = ?")
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        row.map(BranchRow::into_domain).transpose()
    }

    /// Inserta una organización dentro de la transacción abierta.
    pub async fn insert_organization(
        &mut self,
        organization: &Organization,
        occurred_at: &str,
    ) -> Result<(), OrganizationBranchStorageError> {
        sqlx::query(
            "INSERT INTO organization (id, code, name, currency, timezone, locale, stock_negative_policy, status, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(organization.id().as_str())
        .bind(organization.code().as_str())
        .bind(organization.name().as_str())
        .bind(organization.currency().as_str())
        .bind(organization.timezone().as_str())
        .bind(organization.locale().as_str())
        .bind(organization.negative_stock_policy().as_str())
        .bind(organization.status().as_str())
        .bind(revision_to_i64(organization.revision())?)
        .bind(occurred_at)
        .bind(occurred_at)
        .execute(&mut *self.transaction)
        .await
        .map_err(map_organization_write_error)?;
        Ok(())
    }

    /// Actualiza una organización mediante CAS de revisión dentro de la UoW.
    pub async fn update_organization_cas(
        &mut self,
        previous_revision: u64,
        organization: &Organization,
        occurred_at: &str,
    ) -> Result<crate::CasOutcome, OrganizationBranchStorageError> {
        let result = sqlx::query(
            "UPDATE organization SET name = ?, currency = ?, timezone = ?, locale = ?, stock_negative_policy = ?, status = ?, version = ?, updated_at = ? WHERE id = ? AND version = ?",
        )
        .bind(organization.name().as_str())
        .bind(organization.currency().as_str())
        .bind(organization.timezone().as_str())
        .bind(organization.locale().as_str())
        .bind(organization.negative_stock_policy().as_str())
        .bind(organization.status().as_str())
        .bind(revision_to_i64(organization.revision())?)
        .bind(occurred_at)
        .bind(organization.id().as_str())
        .bind(revision_to_i64(previous_revision)?)
        .execute(&mut *self.transaction)
        .await
        .map_err(|_| OrganizationBranchStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(crate::CasOutcome::Updated);
        }
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM organization WHERE id = ?)")
                .bind(organization.id().as_str())
                .fetch_one(&mut *self.transaction)
                .await
                .map_err(|_| OrganizationBranchStorageError::Database)?;
        Ok(if exists == 0 {
            crate::CasOutcome::Missing
        } else {
            crate::CasOutcome::Conflict
        })
    }

    /// Inserta una sucursal dentro de la transacción abierta.
    pub async fn insert_branch(
        &mut self,
        branch: &Branch,
        occurred_at: &str,
    ) -> Result<(), OrganizationBranchStorageError> {
        sqlx::query("INSERT INTO branch (id, organization_id, code, name, status, timezone_override, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(branch.id().as_str())
            .bind(branch.organization_id().as_str())
            .bind(branch.code().as_str())
            .bind(branch.name().as_str())
            .bind(branch.status().as_str())
            .bind(branch.timezone_override().map(pos_core::organization_branch::Timezone::as_str))
            .bind(revision_to_i64(branch.revision())?)
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(map_branch_write_error)?;
        Ok(())
    }

    /// Actualiza con CAS y ownership compuestos dentro de la misma conexión.
    pub async fn update_branch_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        branch: &Branch,
        occurred_at: &str,
    ) -> Result<crate::CasOutcome, OrganizationBranchStorageError> {
        let result = sqlx::query("UPDATE branch SET name = ?, status = ?, timezone_override = ?, version = ?, updated_at = ? WHERE organization_id = ? AND id = ? AND version = ?")
            .bind(branch.name().as_str())
            .bind(branch.status().as_str())
            .bind(branch.timezone_override().map(pos_core::organization_branch::Timezone::as_str))
            .bind(revision_to_i64(branch.revision())?)
            .bind(occurred_at)
            .bind(organization_id.as_str())
            .bind(branch.id().as_str())
            .bind(revision_to_i64(previous_revision)?)
            .execute(&mut *self.transaction)
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(crate::CasOutcome::Updated);
        }
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM branch WHERE organization_id = ? AND id = ?)",
        )
        .bind(organization_id.as_str())
        .bind(branch.id().as_str())
        .fetch_one(&mut *self.transaction)
        .await
        .map_err(|_| OrganizationBranchStorageError::Database)?;
        Ok(if exists == 0 {
            crate::CasOutcome::Missing
        } else {
            crate::CasOutcome::Conflict
        })
    }

    /// Confirma todos los efectos ya ejecutados sobre esta conexión.
    pub async fn commit(self) -> Result<(), OrganizationBranchStorageError> {
        self.transaction
            .commit()
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)
    }

    /// Revierte la transacción abierta; se usa para todo error anterior a commit.
    pub async fn rollback(self) -> Result<(), OrganizationBranchStorageError> {
        self.transaction
            .rollback()
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)
    }

    /// Expone la conexión solo a un writer de auditoría inyectado por el
    /// composition root.
    pub fn connection(&mut self) -> &mut SqliteConnection {
        &mut self.transaction
    }
}

/// Writer de auditoría que participa en la conexión transaccional actual.
///
/// No existe implementación productiva mientras el DDL Local de auditoría no sea
/// aprobado. Los writers de laboratorio deben documentar su tabla fixture.
pub trait TransactionalAuditWriter {
    /// Inserta el evento o devuelve error para forzar rollback.
    fn append(
        &self,
        transaction: &mut SqliteTransaction,
        event: &AuditEvent,
        occurred_at: &str,
    ) -> impl std::future::Future<Output = Result<(), ApplicationError>> + Send;
}
