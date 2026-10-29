//! Adaptador SQLite de foundation; no autoriza importaciones ni escrituras de negocio.

use std::{path::Path, time::Duration};

use pos_core::organization_branch::{
    ApplicationError, AuditEvent, Authorization, Branch, BranchPatch, BusinessCode, DisplayName,
    DomainError, EntityId, IdentityGenerator, IsoCurrency, LocaleTag, NegativeStockPolicy,
    NewBranch, NewOrganization, Organization, OrganizationPatch, Permission, Status, Timezone,
    branch_changed_fields, organization_changed_fields,
};
use sqlx::{
    ConnectOptions, SqlStr, Sqlite, SqliteConnection, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

/// Fallos de infraestructura sin SQL, rutas ni valores de filas en su representación pública.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// La ruta debe designar un archivo y el timeout debe ser explícito y acotado.
    #[error("configuración SQLite inválida")]
    InvalidConfiguration,
    /// No se pudo abrir o utilizar la conexión; no se imprime el error SQL original.
    #[error("operación SQLite fallida")]
    Database,
    /// La conexión no cumple los PRAGMA exigidos para la foundation.
    #[error("configuración efectiva SQLite insegura")]
    UnsafeConnection,
    /// SQLx rechazó la migración, incluido checksum o transacción fallida.
    #[error("migración SQLite rechazada")]
    Migration,
    /// El checkpoint no pudo completarse; el cierre no debe ocultar este resultado.
    #[error("checkpoint SQLite ocupado o incompleto")]
    Checkpoint,
}

/// Abre un archivo local con un único slot de escritura por proceso.
///
/// El timeout es una entrada explícita de laboratorio, no una política medida de producto.
/// No usar sobre NFS/SMB. Un pool de tamaño uno NO establece ownership entre procesos.
pub async fn open(path: &Path, busy_timeout: Duration) -> Result<SqlitePool, StorageError> {
    let name = path.to_str().ok_or(StorageError::InvalidConfiguration)?;
    if name.trim().is_empty()
        || has_disallowed_path_colon(name)
        || has_disallowed_path_question_mark(name)
        || name.contains(['#', '\0'])
        || path.is_dir()
        || busy_timeout < Duration::from_millis(1)
        || busy_timeout > Duration::from_secs(60)
    {
        return Err(StorageError::InvalidConfiguration);
    }
    let expected_timeout =
        i64::try_from(busy_timeout.as_millis()).map_err(|_| StorageError::InvalidConfiguration)?;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(busy_timeout)
        .disable_statement_logging();
    SqlitePoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            Box::pin(async move {
                let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                    .fetch_one(&mut *connection)
                    .await?;
                let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
                    .fetch_one(&mut *connection)
                    .await?;
                let timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
                    .fetch_one(&mut *connection)
                    .await?;
                let durability: i64 = sqlx::query_scalar("PRAGMA synchronous")
                    .fetch_one(&mut *connection)
                    .await?;
                if fk != 1 || journal != "wal" || timeout != expected_timeout || durability != 2 {
                    return Err(sqlx::Error::Protocol(
                        "SQLite PRAGMA verification failed".into(),
                    ));
                }
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .map_err(|_| StorageError::Database)
}

fn has_disallowed_path_colon(value: &str) -> bool {
    if cfg!(windows) {
        let bytes = value.as_bytes();
        let drive_index = if bytes.starts_with(b"\\\\?\\") { 5 } else { 1 };
        bytes.iter().enumerate().any(|(index, byte)| {
            *byte == b':'
                && !(index == drive_index
                    && bytes
                        .get(drive_index.saturating_sub(1))
                        .is_some_and(u8::is_ascii_alphabetic))
        })
    } else {
        value.contains(':')
    }
}

fn has_disallowed_path_question_mark(value: &str) -> bool {
    if cfg!(windows) && value.as_bytes().starts_with(b"\\\\?\\") {
        value.as_bytes()[3..].contains(&b'?')
    } else {
        value.contains('?')
    }
}

/// Aplica exclusivamente el root Local embebido; el esquema sigue siendo parcial.
///
/// SQLx conserva su propio historial y rechaza checksums alterados. Nunca se modifica Flyway.
pub async fn migrate(pool: &SqlitePool) -> Result<(), StorageError> {
    sqlx::migrate!("../../migrations/local")
        .run(pool)
        .await
        .map_err(|_| StorageError::Migration)
}

/// Solicita checkpoint después de drenar operaciones; no constituye un backup.
pub async fn checkpoint(pool: &SqlitePool) -> Result<(), StorageError> {
    let (busy, _, _): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(pool)
        .await
        .map_err(|_| StorageError::Database)?;
    if busy != 0 {
        return Err(StorageError::Checkpoint);
    }
    Ok(())
}

/// Resultado de una actualización CAS scoped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CasOutcome {
    /// La fila fue actualizada y su versión ya fue incrementada por el dominio.
    Updated,
    /// No existe una fila en el scope solicitado.
    Missing,
    /// Existe una fila en el scope, pero su versión ya no coincide.
    Conflict,
}

/// Error tipado del adaptador experimental Organization/Branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OrganizationBranchStorageError {
    /// Un código de organización ya está ocupado.
    #[error("código de organización duplicado")]
    DuplicateOrganizationCode,
    /// Un código de sucursal ya está ocupado dentro de su organización.
    #[error("código de sucursal duplicado")]
    DuplicateBranchCode,
    /// La organización referenciada por la sucursal no existe.
    #[error("organización referenciada inexistente")]
    ForeignKey,
    /// Una fila de almacenamiento no cumple los invariantes del núcleo.
    #[error("fila Organization/Branch inválida")]
    InvalidRow,
    /// La operación SQLite falló sin exponer SQL ni valores.
    #[error("operación SQLite Organization/Branch fallida")]
    Database,
}

impl From<DomainError> for OrganizationBranchStorageError {
    fn from(_: DomainError) -> Self {
        Self::InvalidRow
    }
}

/// Adaptador SQLite experimental para el subconjunto Organization/Branch.
///
/// No es un adaptador productivo: el root Local no contiene aún el esquema de
/// auditoría aprobado. Todos los métodos de sucursal exigen `organization_id`.
#[derive(Clone)]
pub struct SqliteOrganizationBranchStore {
    pool: SqlitePool,
}

impl SqliteOrganizationBranchStore {
    /// Construye el adaptador sobre un pool ya migrado por [`migrate`].
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserta una organización usando parámetros SQL.
    pub async fn insert_organization(
        &self,
        organization: &Organization,
        created_at: &str,
        updated_at: &str,
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
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await
        .map_err(map_organization_write_error)?;
        Ok(())
    }

    /// Lee y revalida una organización por UUID.
    pub async fn get_organization(
        &self,
        organization_id: &EntityId,
    ) -> Result<Option<Organization>, OrganizationBranchStorageError> {
        let row = sqlx::query_as::<_, OrganizationRow>("SELECT id, code, name, currency, timezone, locale, stock_negative_policy, status, version FROM organization WHERE id = ?")
            .bind(organization_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        row.map(OrganizationRow::into_domain).transpose()
    }

    /// Actualiza todos los campos mutables mediante CAS de versión.
    pub async fn update_organization_cas(
        &self,
        previous_revision: u64,
        organization: &Organization,
        updated_at: &str,
    ) -> Result<CasOutcome, OrganizationBranchStorageError> {
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
        .bind(updated_at)
        .bind(organization.id().as_str())
        .bind(revision_to_i64(previous_revision)?)
        .execute(&self.pool)
        .await
        .map_err(|_| OrganizationBranchStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM organization WHERE id = ?)")
                .bind(organization.id().as_str())
                .fetch_one(&self.pool)
                .await
                .map_err(|_| OrganizationBranchStorageError::Database)?;
        Ok(if exists == 0 {
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
        })
    }

    /// Inserta una sucursal; la FK se verifica por SQLite real.
    pub async fn insert_branch(
        &self,
        branch: &Branch,
        created_at: &str,
        updated_at: &str,
    ) -> Result<(), OrganizationBranchStorageError> {
        sqlx::query(
            "INSERT INTO branch (id, organization_id, code, name, status, timezone_override, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(branch.id().as_str())
        .bind(branch.organization_id().as_str())
        .bind(branch.code().as_str())
        .bind(branch.name().as_str())
        .bind(branch.status().as_str())
        .bind(branch.timezone_override().map(Timezone::as_str))
        .bind(revision_to_i64(branch.revision())?)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await
        .map_err(map_branch_write_error)?;
        Ok(())
    }

    /// Lee una sucursal solo dentro de la organización indicada.
    pub async fn get_branch(
        &self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Option<Branch>, OrganizationBranchStorageError> {
        let row = sqlx::query_as::<_, BranchRow>("SELECT id, organization_id, code, name, status, timezone_override, version FROM branch WHERE organization_id = ? AND id = ?")
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        row.map(BranchRow::into_domain).transpose()
    }

    /// Lista únicamente sucursales del owner solicitado.
    pub async fn list_branches(
        &self,
        organization_id: &EntityId,
    ) -> Result<Vec<Branch>, OrganizationBranchStorageError> {
        let rows = sqlx::query_as::<_, BranchRow>("SELECT id, organization_id, code, name, status, timezone_override, version FROM branch WHERE organization_id = ? ORDER BY code")
            .bind(organization_id.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        rows.into_iter().map(BranchRow::into_domain).collect()
    }

    /// Actualiza una sucursal usando owner, UUID y versión como condición CAS.
    pub async fn update_branch_cas(
        &self,
        organization_id: &EntityId,
        previous_revision: u64,
        branch: &Branch,
        updated_at: &str,
    ) -> Result<CasOutcome, OrganizationBranchStorageError> {
        let result = sqlx::query(
            "UPDATE branch SET name = ?, status = ?, timezone_override = ?, version = ?, updated_at = ? WHERE organization_id = ? AND id = ? AND version = ?",
        )
        .bind(branch.name().as_str())
        .bind(branch.status().as_str())
        .bind(branch.timezone_override().map(Timezone::as_str))
        .bind(revision_to_i64(branch.revision())?)
        .bind(updated_at)
        .bind(organization_id.as_str())
        .bind(branch.id().as_str())
        .bind(revision_to_i64(previous_revision)?)
        .execute(&self.pool)
        .await
        .map_err(|_| OrganizationBranchStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM branch WHERE organization_id = ? AND id = ?)",
        )
        .bind(organization_id.as_str())
        .bind(branch.id().as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|_| OrganizationBranchStorageError::Database)?;
        Ok(if exists == 0 {
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
        })
    }
}

#[derive(sqlx::FromRow)]
struct OrganizationRow {
    id: String,
    code: String,
    name: String,
    currency: String,
    timezone: String,
    locale: String,
    stock_negative_policy: String,
    status: String,
    version: i64,
}

impl OrganizationRow {
    fn into_domain(self) -> Result<Organization, OrganizationBranchStorageError> {
        let revision =
            u64::try_from(self.version).map_err(|_| OrganizationBranchStorageError::InvalidRow)?;
        Ok(Organization::rehydrate(
            EntityId::parse(&self.id)?,
            DisplayName::new(&self.name)?,
            BusinessCode::new(&self.code)?,
            Timezone::new(&self.timezone)?,
            IsoCurrency::new(&self.currency)?,
            LocaleTag::new(&self.locale)?,
            Status::parse(&self.status)?,
            NegativeStockPolicy::parse(&self.stock_negative_policy)?,
            revision,
        ))
    }
}

#[derive(sqlx::FromRow)]
struct BranchRow {
    id: String,
    organization_id: String,
    code: String,
    name: String,
    status: String,
    timezone_override: Option<String>,
    version: i64,
}

impl BranchRow {
    fn into_domain(self) -> Result<Branch, OrganizationBranchStorageError> {
        let revision =
            u64::try_from(self.version).map_err(|_| OrganizationBranchStorageError::InvalidRow)?;
        let timezone_override = self
            .timezone_override
            .map(|value| Timezone::new(&value))
            .transpose()?;
        Ok(Branch::rehydrate(
            EntityId::parse(&self.id)?,
            EntityId::parse(&self.organization_id)?,
            DisplayName::new(&self.name)?,
            BusinessCode::new(&self.code)?,
            timezone_override,
            Status::parse(&self.status)?,
            revision,
        ))
    }
}

fn revision_to_i64(revision: u64) -> Result<i64, OrganizationBranchStorageError> {
    i64::try_from(revision).map_err(|_| OrganizationBranchStorageError::InvalidRow)
}

fn map_organization_write_error(error: sqlx::Error) -> OrganizationBranchStorageError {
    if error
        .as_database_error()
        .is_some_and(|database| database.is_unique_violation())
    {
        OrganizationBranchStorageError::DuplicateOrganizationCode
    } else {
        OrganizationBranchStorageError::Database
    }
}

fn map_branch_write_error(error: sqlx::Error) -> OrganizationBranchStorageError {
    if error
        .as_database_error()
        .is_some_and(|database| database.is_unique_violation())
    {
        OrganizationBranchStorageError::DuplicateBranchCode
    } else if error
        .as_database_error()
        .is_some_and(|database| database.is_foreign_key_violation())
    {
        OrganizationBranchStorageError::ForeignKey
    } else {
        OrganizationBranchStorageError::Database
    }
}

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
    /// `BEGIN IMMEDIATE` se entrega a la construcción nativa de
    /// [`Transaction`], por lo que conserva su rollback RAII si el futuro se
    /// cancela o el valor se abandona antes de confirmar. La sentencia es
    /// estática; no se construye a partir de datos de entrada.
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

    /// Lee sin abrir una transacción de escritura; las lecturas no deben tomar
    /// la reserva inmediata destinada a mutaciones auditables.
    pub async fn get_organization(
        &self,
        organization_id: &EntityId,
    ) -> Result<Option<Organization>, OrganizationBranchStorageError> {
        SqliteOrganizationBranchStore::new(self.pool.clone())
            .get_organization(organization_id)
            .await
    }
}

/// Transacción Local con ownership exclusivo de la conexión SQLx.
///
/// No clona conexiones ni usa `block_on`. `sqlx::Transaction` hace rollback
/// al abandonarse sin commit; los métodos nunca usan el pool mientras esté activa.
pub struct SqliteTransaction {
    transaction: Transaction<'static, Sqlite>,
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
    ) -> Result<CasOutcome, OrganizationBranchStorageError> {
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
            return Ok(CasOutcome::Updated);
        }
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM organization WHERE id = ?)")
                .bind(organization.id().as_str())
                .fetch_one(&mut *self.transaction)
                .await
                .map_err(|_| OrganizationBranchStorageError::Database)?;
        Ok(if exists == 0 {
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
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
            .bind(branch.timezone_override().map(Timezone::as_str))
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
    ) -> Result<CasOutcome, OrganizationBranchStorageError> {
        let result = sqlx::query("UPDATE branch SET name = ?, status = ?, timezone_override = ?, version = ?, updated_at = ? WHERE organization_id = ? AND id = ? AND version = ?")
            .bind(branch.name().as_str())
            .bind(branch.status().as_str())
            .bind(branch.timezone_override().map(Timezone::as_str))
            .bind(revision_to_i64(branch.revision())?)
            .bind(occurred_at)
            .bind(organization_id.as_str())
            .bind(branch.id().as_str())
            .bind(revision_to_i64(previous_revision)?)
            .execute(&mut *self.transaction)
            .await
            .map_err(|_| OrganizationBranchStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
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
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
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

    /// Expone la conexión solo a un writer de auditoría inyectado por el composition root.
    pub fn connection(&mut self) -> &mut SqliteConnection {
        &mut self.transaction
    }
}

/// Writer de auditoría que participa en la conexión transaccional actual.
///
/// No existe implementación productiva mientras el DDL Local de auditoría no
/// sea aprobado. Los writers de laboratorio deben documentar su tabla fixture.
pub trait TransactionalAuditWriter {
    /// Inserta el evento o devuelve error para forzar rollback.
    fn append(
        &self,
        transaction: &mut SqliteTransaction,
        event: &AuditEvent,
        occurred_at: &str,
    ) -> impl std::future::Future<Output = Result<(), ApplicationError>> + Send;
}

/// Composición asíncrona experimental de Organization contra SQLite real.
///
/// La creación representa únicamente el tramo Organization del bootstrap Java:
/// exige una capacidad de bootstrap explícita, registra `organization.created`
/// y comparte la transacción con el writer. No sustituye el guard secreto, el
/// estado one-shot ni el provisioning de usuario/rol que todavía no existen en
/// el esquema Local.
pub struct SqliteOrganizationApplication<A> {
    unit_of_work: SqliteUnitOfWork,
    audit: A,
}

impl<A: TransactionalAuditWriter> SqliteOrganizationApplication<A> {
    /// Ensambla la composición experimental sin habilitar bootstrap por defecto.
    pub fn new(unit_of_work: SqliteUnitOfWork, audit: A) -> Self {
        Self {
            unit_of_work,
            audit,
        }
    }

    /// Crea la organización durante un bootstrap previamente autorizado.
    pub async fn bootstrap_create<I: IdentityGenerator, Z: Authorization>(
        &self,
        authorization: &Z,
        identities: &mut I,
        input: NewOrganization,
        occurred_at: &str,
    ) -> Result<Organization, ApplicationError> {
        if !authorization.is_allowed(Permission::BootstrapOrganization, None, None) {
            return Err(ApplicationError::Forbidden);
        }
        let id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(storage_error)?;
        let operation = async {
            let organization = Organization::create(id, input);
            transaction
                .insert_organization(&organization, occurred_at)
                .await
                .map_err(storage_error)?;
            self.audit
                .append(
                    &mut transaction,
                    &AuditEvent {
                        organization_id: organization.id().clone(),
                        branch_id: None,
                        action: "organization.created",
                        entity_type: "organization",
                        entity_id: organization.id().clone(),
                        changed_fields: Vec::new(),
                    },
                    occurred_at,
                )
                .await?;
            Ok(organization)
        }
        .await;
        match operation {
            Ok(organization) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(organization)
            }
            Err(error) => {
                if transaction.rollback().await.is_err() {
                    return Err(ApplicationError::Storage);
                }
                Err(error)
            }
        }
    }

    /// Obtiene una organización sin tomar un lock de escritor.
    pub async fn get<Z: Authorization>(
        &self,
        authorization: &Z,
        organization_id: &EntityId,
    ) -> Result<Organization, ApplicationError> {
        if !authorization.is_allowed(Permission::OrganizationRead, Some(organization_id), None) {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work
            .get_organization(organization_id)
            .await
            .map_err(storage_error)?
            .ok_or(ApplicationError::OrganizationNotFound)
    }

    /// Aplica un patch CAS y registra solo una modificación material.
    pub async fn update<Z: Authorization>(
        &self,
        authorization: &Z,
        organization_id: &EntityId,
        patch: OrganizationPatch,
        occurred_at: &str,
    ) -> Result<Organization, ApplicationError> {
        if !authorization.is_allowed(Permission::OrganizationUpdate, Some(organization_id), None) {
            return Err(ApplicationError::Forbidden);
        }
        let mut transaction = self.unit_of_work.begin().await.map_err(storage_error)?;
        let operation = async {
            let mut organization = transaction
                .get_organization(organization_id)
                .await
                .map_err(storage_error)?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            let before = organization.clone();
            let previous_revision = organization.revision();
            if !organization.patch(patch)? {
                return Ok(organization);
            }
            if transaction
                .update_organization_cas(previous_revision, &organization, occurred_at)
                .await
                .map_err(storage_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            self.audit
                .append(
                    &mut transaction,
                    &AuditEvent {
                        organization_id: organization_id.clone(),
                        branch_id: None,
                        action: "organization.updated",
                        entity_type: "organization",
                        entity_id: organization_id.clone(),
                        changed_fields: organization_changed_fields(&before, &organization),
                    },
                    occurred_at,
                )
                .await?;
            Ok(organization)
        }
        .await;
        match operation {
            Ok(organization) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(organization)
            }
            Err(error) => {
                if transaction.rollback().await.is_err() {
                    return Err(ApplicationError::Storage);
                }
                Err(error)
            }
        }
    }
}

/// Composición asíncrona experimental de los casos Branch contra SQLite real.
///
/// La autorización se evalúa antes de abrir la transacción; no hay política
/// implícita. La mutación, CAS y writer de auditoría comparten una conexión.
pub struct SqliteBranchApplication<A> {
    unit_of_work: SqliteUnitOfWork,
    audit: A,
}

impl<A: TransactionalAuditWriter> SqliteBranchApplication<A> {
    /// Ensambla la composición experimental sin adaptador de autorización por defecto.
    pub fn new(unit_of_work: SqliteUnitOfWork, audit: A) -> Self {
        Self {
            unit_of_work,
            audit,
        }
    }

    /// Crea una Branch y registra el evento en la misma transacción SQLite.
    pub async fn create<I: IdentityGenerator, Z: Authorization>(
        &self,
        authorization: &Z,
        identities: &mut I,
        organization_id: &EntityId,
        input: NewBranch,
        occurred_at: &str,
    ) -> Result<Branch, ApplicationError> {
        if !authorization.is_allowed(Permission::BranchCreate, Some(organization_id), None) {
            return Err(ApplicationError::Forbidden);
        }
        let id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(storage_error)?;
        let operation = async {
            if transaction
                .get_organization(organization_id)
                .await
                .map_err(storage_error)?
                .is_none()
            {
                return Err(ApplicationError::OrganizationNotFound);
            }
            let branch = Branch::create(id, organization_id.clone(), input);
            transaction
                .insert_branch(&branch, occurred_at)
                .await
                .map_err(storage_error)?;
            self.audit
                .append(
                    &mut transaction,
                    &AuditEvent {
                        organization_id: organization_id.clone(),
                        branch_id: Some(branch.id().clone()),
                        action: "branch.created",
                        entity_type: "branch",
                        entity_id: branch.id().clone(),
                        changed_fields: Vec::new(),
                    },
                    occurred_at,
                )
                .await?;
            Ok(branch)
        }
        .await;
        match operation {
            Ok(branch) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(branch)
            }
            Err(error) => {
                if transaction.rollback().await.is_err() {
                    return Err(ApplicationError::Storage);
                }
                Err(error)
            }
        }
    }

    /// Aplica CAS scoped, omite auditoría para no-op y confirma una sola vez.
    pub async fn update<Z: Authorization>(
        &self,
        authorization: &Z,
        organization_id: &EntityId,
        branch_id: &EntityId,
        patch: BranchPatch,
        occurred_at: &str,
    ) -> Result<Branch, ApplicationError> {
        if !authorization.is_allowed(
            Permission::BranchUpdate,
            Some(organization_id),
            Some(branch_id),
        ) {
            return Err(ApplicationError::Forbidden);
        }
        let mut transaction = self.unit_of_work.begin().await.map_err(storage_error)?;
        let operation = async {
            if transaction
                .get_organization(organization_id)
                .await
                .map_err(storage_error)?
                .is_none()
            {
                return Err(ApplicationError::OrganizationNotFound);
            }
            let mut branch = transaction
                .get_branch(organization_id, branch_id)
                .await
                .map_err(storage_error)?
                .ok_or(ApplicationError::BranchNotFound)?;
            let before = branch.clone();
            let previous_revision = branch.revision();
            if !branch.patch(organization_id, patch)? {
                return Ok(branch);
            }
            if transaction
                .update_branch_cas(organization_id, previous_revision, &branch, occurred_at)
                .await
                .map_err(storage_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            let changed_fields = branch_changed_fields(&before, &branch);
            self.audit
                .append(
                    &mut transaction,
                    &AuditEvent {
                        organization_id: organization_id.clone(),
                        branch_id: Some(branch_id.clone()),
                        action: "branch.updated",
                        entity_type: "branch",
                        entity_id: branch_id.clone(),
                        changed_fields,
                    },
                    occurred_at,
                )
                .await?;
            Ok(branch)
        }
        .await;
        match operation {
            Ok(branch) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(branch)
            }
            Err(error) => {
                if transaction.rollback().await.is_err() {
                    return Err(ApplicationError::Storage);
                }
                Err(error)
            }
        }
    }
}

fn storage_error(error: OrganizationBranchStorageError) -> ApplicationError {
    match error {
        OrganizationBranchStorageError::DuplicateBranchCode => {
            ApplicationError::BranchCodeAlreadyExists
        }
        OrganizationBranchStorageError::DuplicateOrganizationCode => {
            ApplicationError::OrganizationCodeAlreadyExists
        }
        _ => ApplicationError::Storage,
    }
}

#[cfg(test)]
mod tests;
