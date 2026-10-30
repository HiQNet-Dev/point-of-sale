//! Casos de uso SQLite candidatos de cajas y turnos.
//!
//! **No productivo todavía.** Las tablas viven en `proposals/local-cash/` y no
//! están cargadas por `pos-local`. Esta composición reutiliza la frontera de
//! principal verificado, `BEGIN IMMEDIATE` y auditoría transaccional para que la
//! promoción aprobada no requiera reescribir reglas de seguridad.
//!
//! El puerto [`OpenSalesForCashShift`] se ejecuta dentro de la misma UoW que el
//! cierre. La futura persistencia de ventas debe usar la misma estrategia de
//! serialización para que crear una venta y cerrar su turno sean una decisión
//! atómica, nunca un `SELECT` rancio seguido de un `UPDATE`.

use std::future::Future;

use pos_application::{
    audit::{append_audit, append_audit_with_id},
    authorization::require_branch_scope,
};
use pos_core::{
    audit::AuditOccurredAt,
    cash::{
        CashError, CashRegister, CashRegisterPatch, CashShift, CashShiftStatus, CloseCashShift,
        NewCashRegister, NewCashShift, cash_register_changed_fields, ensure_close_allowed,
        ensure_open_allowed,
    },
    catalog::{CatalogCode, UnitPrice},
    identity::{ScopeMode, UtcSeconds, VerifiedPrincipal},
    organization_branch::{
        ApplicationError, DisplayName, EntityId, IdentityGenerator, Permission, Status,
    },
};
use serde_json::json;

use crate::{CasOutcome, SqliteTransaction, SqliteUnitOfWork};

const SELECT_REGISTER: &str = "SELECT id, organization_id, branch_id, code, name, status, version \
    FROM cash_register WHERE organization_id = ? AND branch_id = ? AND id = ?";
const LIST_REGISTERS: &str = "SELECT id, organization_id, branch_id, code, name, status, version \
    FROM cash_register WHERE organization_id = ? AND branch_id = ? AND (? IS NULL OR status = ?) \
    ORDER BY name, id LIMIT ? OFFSET ?";
const REGISTER_CODE_EXISTS: &str = "SELECT EXISTS(SELECT 1 FROM cash_register \
    WHERE organization_id = ? AND branch_id = ? AND code = ?)";
const INSERT_REGISTER: &str = "INSERT INTO cash_register \
    (id, organization_id, branch_id, code, name, status, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";
const UPDATE_REGISTER_CAS: &str = "UPDATE cash_register SET name = ?, status = ?, version = ?, updated_at = ? \
    WHERE organization_id = ? AND branch_id = ? AND id = ? AND version = ?";
const SELECT_REGISTER_VERSION: &str = "SELECT version FROM cash_register \
    WHERE organization_id = ? AND branch_id = ? AND id = ?";

const SELECT_SHIFT: &str = "SELECT id, organization_id, branch_id, cash_register_id, opened_by_user_id, \
    closed_by_user_id, status, opening_amount, closing_declared_amount, \
    CAST(strftime('%s', opened_at) AS INTEGER) AS opened_at, \
    CAST(strftime('%s', closed_at) AS INTEGER) AS closed_at, version \
    FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND id = ?";
const SELECT_OPEN_SHIFT: &str = "SELECT id, organization_id, branch_id, cash_register_id, opened_by_user_id, \
    closed_by_user_id, status, opening_amount, closing_declared_amount, \
    CAST(strftime('%s', opened_at) AS INTEGER) AS opened_at, \
    CAST(strftime('%s', closed_at) AS INTEGER) AS closed_at, version \
    FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? \
    AND status = 'OPEN' ORDER BY opened_at DESC, id DESC LIMIT 1";
const LIST_SHIFTS: &str = "SELECT id, organization_id, branch_id, cash_register_id, opened_by_user_id, \
    closed_by_user_id, status, opening_amount, closing_declared_amount, \
    CAST(strftime('%s', opened_at) AS INTEGER) AS opened_at, \
    CAST(strftime('%s', closed_at) AS INTEGER) AS closed_at, version \
    FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? \
    AND (? IS NULL OR status = ?) ORDER BY opened_at DESC, id DESC LIMIT ? OFFSET ?";
const OPEN_SHIFT_EXISTS: &str = "SELECT EXISTS(SELECT 1 FROM cash_shift \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND status = 'OPEN')";
const INSERT_SHIFT: &str = "INSERT INTO cash_shift \
    (id, organization_id, branch_id, cash_register_id, opened_by_user_id, closed_by_user_id, \
     status, opening_amount, closing_declared_amount, opened_at, closed_at, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, NULL, ?, ?, NULL, ?, NULL, ?, ?, ?)";
const UPDATE_SHIFT_CAS: &str = "UPDATE cash_shift SET closed_by_user_id = ?, status = ?, \
    closing_declared_amount = ?, closed_at = ?, version = ?, updated_at = ? \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND id = ? AND version = ? \
    AND status = 'OPEN'";
const SELECT_SHIFT_VERSION: &str = "SELECT version FROM cash_shift \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND id = ?";

/// Límite local de lecturas paginadas de cajas y turnos.
pub const MAX_CASH_PAGE_SIZE: u32 = 100;

/// Error de persistencia candidato sin exponer SQL ni valores de entrada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CashStorageError {
    /// Código de caja repetido en la misma sucursal.
    DuplicateRegisterCode,
    /// El índice parcial rechazó una segunda apertura concurrente.
    OpenShiftExists,
    /// Una FK de ownership o actor fue rechazada.
    ForeignKey,
    /// La fila incumple un CHECK o no puede rehidratarse.
    InvalidRow,
    /// SQLite rechazó la operación por una causa no clasificable.
    Database,
}

impl std::fmt::Display for CashStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::DuplicateRegisterCode => "código de caja duplicado",
            Self::OpenShiftExists => "la caja ya tiene un turno abierto",
            Self::ForeignKey => "referencia de caja o turno inválida",
            Self::InvalidRow => "fila de caja o turno inválida",
            Self::Database => "operación SQLite de caja o turno fallida",
        })
    }
}
impl std::error::Error for CashStorageError {}

#[derive(sqlx::FromRow)]
struct CashRegisterRow {
    id: String,
    organization_id: String,
    branch_id: String,
    code: String,
    name: String,
    status: String,
    version: i64,
}

impl CashRegisterRow {
    fn into_domain(self) -> Result<CashRegister, CashStorageError> {
        Ok(CashRegister::restore(
            EntityId::parse(&self.id).map_err(|_| CashStorageError::InvalidRow)?,
            EntityId::parse(&self.organization_id).map_err(|_| CashStorageError::InvalidRow)?,
            EntityId::parse(&self.branch_id).map_err(|_| CashStorageError::InvalidRow)?,
            CatalogCode::restore(&self.code).map_err(|_| CashStorageError::InvalidRow)?,
            DisplayName::new(&self.name).map_err(|_| CashStorageError::InvalidRow)?,
            Status::parse(&self.status).map_err(|_| CashStorageError::InvalidRow)?,
            revision_to_u64(self.version)?,
        ))
    }
}

#[derive(sqlx::FromRow)]
struct CashShiftRow {
    id: String,
    organization_id: String,
    branch_id: String,
    cash_register_id: String,
    opened_by_user_id: String,
    closed_by_user_id: Option<String>,
    status: String,
    opening_amount: String,
    closing_declared_amount: Option<String>,
    opened_at: i64,
    closed_at: Option<i64>,
    version: i64,
}

impl CashShiftRow {
    fn into_domain(self) -> Result<CashShift, CashStorageError> {
        CashShift::restore(
            EntityId::parse(&self.id).map_err(|_| CashStorageError::InvalidRow)?,
            EntityId::parse(&self.organization_id).map_err(|_| CashStorageError::InvalidRow)?,
            EntityId::parse(&self.branch_id).map_err(|_| CashStorageError::InvalidRow)?,
            EntityId::parse(&self.cash_register_id).map_err(|_| CashStorageError::InvalidRow)?,
            EntityId::parse(&self.opened_by_user_id).map_err(|_| CashStorageError::InvalidRow)?,
            self.closed_by_user_id
                .as_deref()
                .map(EntityId::parse)
                .transpose()
                .map_err(|_| CashStorageError::InvalidRow)?,
            CashShiftStatus::parse(&self.status).map_err(|_| CashStorageError::InvalidRow)?,
            UnitPrice::parse(&self.opening_amount).map_err(|_| CashStorageError::InvalidRow)?,
            self.closing_declared_amount
                .as_deref()
                .map(UnitPrice::parse)
                .transpose()
                .map_err(|_| CashStorageError::InvalidRow)?,
            UtcSeconds::new(self.opened_at).map_err(|_| CashStorageError::InvalidRow)?,
            self.closed_at
                .map(UtcSeconds::new)
                .transpose()
                .map_err(|_| CashStorageError::InvalidRow)?,
            revision_to_u64(self.version)?,
        )
        .map_err(|_| CashStorageError::InvalidRow)
    }
}

fn revision_to_u64(value: i64) -> Result<u64, CashStorageError> {
    u64::try_from(value).map_err(|_| CashStorageError::InvalidRow)
}

fn revision_to_i64(value: u64) -> Result<i64, CashStorageError> {
    i64::try_from(value).map_err(|_| CashStorageError::InvalidRow)
}

fn map_write_error(error: &sqlx::Error) -> CashStorageError {
    let Some(database) = error.as_database_error() else {
        return CashStorageError::Database;
    };
    if database.is_unique_violation() {
        let message = database.message();
        if message.contains("cash_register.branch_id, cash_register.code") {
            return CashStorageError::DuplicateRegisterCode;
        }
        if message.contains("cash_shift.cash_register_id") {
            return CashStorageError::OpenShiftExists;
        }
    }
    if database.is_foreign_key_violation() {
        return CashStorageError::ForeignKey;
    }
    if database.is_check_violation() {
        return CashStorageError::InvalidRow;
    }
    CashStorageError::Database
}

impl SqliteTransaction {
    /// Busca una caja únicamente dentro de su organización y sucursal.
    pub async fn get_cash_register(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> Result<Option<CashRegister>, CashStorageError> {
        let row = sqlx::query_as::<_, CashRegisterRow>(SELECT_REGISTER)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(cash_register_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        row.map(CashRegisterRow::into_domain).transpose()
    }

    /// Lista cajas scoped con orden determinista equivalente al controlador Java.
    pub async fn list_cash_registers(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CashRegister>, CashStorageError> {
        let status = status.map(Status::as_str);
        let rows = sqlx::query_as::<_, CashRegisterRow>(LIST_REGISTERS)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(status)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        rows.into_iter().map(CashRegisterRow::into_domain).collect()
    }

    /// Comprueba una colisión de código para una respuesta temprana; el UNIQUE es autoridad final.
    pub async fn cash_register_code_exists(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        code: &CatalogCode,
    ) -> Result<bool, CashStorageError> {
        let exists: i64 = sqlx::query_scalar(REGISTER_CODE_EXISTS)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(code.as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        Ok(exists != 0)
    }

    /// Inserta una caja en la transacción activa.
    pub async fn insert_cash_register(
        &mut self,
        register: &CashRegister,
        occurred_at: &str,
    ) -> Result<(), CashStorageError> {
        sqlx::query(INSERT_REGISTER)
            .bind(register.id().as_str())
            .bind(register.organization_id().as_str())
            .bind(register.branch_id().as_str())
            .bind(register.code().as_str())
            .bind(register.name().as_str())
            .bind(register.status().as_str())
            .bind(revision_to_i64(register.revision())?)
            .bind(occurred_at)
            .bind(occurred_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?;
        Ok(())
    }

    /// Actualiza una caja por scope y versión previa.
    pub async fn update_cash_register_cas(
        &mut self,
        previous_revision: u64,
        register: &CashRegister,
        occurred_at: &str,
    ) -> Result<CasOutcome, CashStorageError> {
        let result = sqlx::query(UPDATE_REGISTER_CAS)
            .bind(register.name().as_str())
            .bind(register.status().as_str())
            .bind(revision_to_i64(register.revision())?)
            .bind(occurred_at)
            .bind(register.organization_id().as_str())
            .bind(register.branch_id().as_str())
            .bind(register.id().as_str())
            .bind(revision_to_i64(previous_revision)?)
            .execute(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let exists: i64 = sqlx::query_scalar(SELECT_REGISTER_VERSION)
            .bind(register.organization_id().as_str())
            .bind(register.branch_id().as_str())
            .bind(register.id().as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        Ok(if exists == 0 {
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
        })
    }

    /// Busca el turno activo de una caja dentro de su scope.
    pub async fn get_open_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> Result<Option<CashShift>, CashStorageError> {
        let row = sqlx::query_as::<_, CashShiftRow>(SELECT_OPEN_SHIFT)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(cash_register_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        row.map(CashShiftRow::into_domain).transpose()
    }

    /// Busca un turno histórico con el scope completo.
    pub async fn get_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
    ) -> Result<Option<CashShift>, CashStorageError> {
        let row = sqlx::query_as::<_, CashShiftRow>(SELECT_SHIFT)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(cash_register_id.as_str())
            .bind(cash_shift_id.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        row.map(CashShiftRow::into_domain).transpose()
    }

    /// Lista el historial de turnos de una caja con orden determinista.
    pub async fn list_cash_shifts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        status: Option<CashShiftStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CashShift>, CashStorageError> {
        let status = status.map(CashShiftStatus::as_str);
        let rows = sqlx::query_as::<_, CashShiftRow>(LIST_SHIFTS)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(cash_register_id.as_str())
            .bind(status)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        rows.into_iter().map(CashShiftRow::into_domain).collect()
    }

    /// Precomprobación UX; el índice parcial de la propuesta sigue siendo autoridad final.
    pub async fn cash_register_has_open_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> Result<bool, CashStorageError> {
        let exists: i64 = sqlx::query_scalar(OPEN_SHIFT_EXISTS)
            .bind(organization_id.as_str())
            .bind(branch_id.as_str())
            .bind(cash_register_id.as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        Ok(exists != 0)
    }

    /// Convierte el RFC 3339 candidato al mismo segundo Unix que conserva el dominio.
    pub async fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> Result<UtcSeconds, CashStorageError> {
        let seconds = sqlx::query_scalar::<_, i64>("SELECT CAST(strftime('%s', ?) AS INTEGER)")
            .bind(occurred_at.as_str())
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?
            .ok_or(CashStorageError::InvalidRow)?;
        UtcSeconds::new(seconds).map_err(|_| CashStorageError::InvalidRow)
    }

    /// Inserta un turno OPEN; el índice parcial resuelve carreras de doble apertura.
    pub async fn insert_cash_shift(
        &mut self,
        shift: &CashShift,
        opened_at: &str,
    ) -> Result<(), CashStorageError> {
        sqlx::query(INSERT_SHIFT)
            .bind(shift.id().as_str())
            .bind(shift.organization_id().as_str())
            .bind(shift.branch_id().as_str())
            .bind(shift.cash_register_id().as_str())
            .bind(shift.opened_by_user_id().as_str())
            .bind(shift.status().as_str())
            .bind(shift.opening_amount().to_string())
            .bind(opened_at)
            .bind(revision_to_i64(shift.revision())?)
            .bind(opened_at)
            .bind(opened_at)
            .execute(&mut *self.transaction)
            .await
            .map_err(|error| map_write_error(&error))?;
        Ok(())
    }

    /// Cierra condicionalmente un turno OPEN por scope y versión previa.
    pub async fn close_cash_shift_cas(
        &mut self,
        previous_revision: u64,
        shift: &CashShift,
        occurred_at: &str,
    ) -> Result<CasOutcome, CashStorageError> {
        let declared = shift
            .closing_declared_amount()
            .ok_or(CashStorageError::InvalidRow)?
            .to_string();
        let actor = shift
            .closed_by_user_id()
            .ok_or(CashStorageError::InvalidRow)?;
        let result = sqlx::query(UPDATE_SHIFT_CAS)
            .bind(actor.as_str())
            .bind(shift.status().as_str())
            .bind(declared)
            .bind(occurred_at)
            .bind(revision_to_i64(shift.revision())?)
            .bind(occurred_at)
            .bind(shift.organization_id().as_str())
            .bind(shift.branch_id().as_str())
            .bind(shift.cash_register_id().as_str())
            .bind(shift.id().as_str())
            .bind(revision_to_i64(previous_revision)?)
            .execute(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let exists: i64 = sqlx::query_scalar(SELECT_SHIFT_VERSION)
            .bind(shift.organization_id().as_str())
            .bind(shift.branch_id().as_str())
            .bind(shift.cash_register_id().as_str())
            .bind(shift.id().as_str())
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| CashStorageError::Database)?;
        Ok(if exists == 0 {
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
        })
    }
}

/// Puerto de ventas para el guard de cierre de turno.
///
/// La implementación debe consultar la misma conexión transaccional recibida y
/// no puede abrir un pool ni responder por defecto `false`. Cuando exista la
/// persistencia de ventas, su creación debe entrar por la misma UoW/serialización
/// antes de insertar una venta OPEN.
pub trait OpenSalesForCashShift {
    /// Indica si el turno posee ventas abiertas dentro de esta transacción.
    fn has_open_sales(
        &self,
        transaction: &mut SqliteTransaction,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<bool, CashStorageError>> + Send;
}

/// Datos de creación de una caja recibidos desde la frontera de aplicación.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCashRegisterRequest {
    /// Código que se normaliza como slug Java.
    pub code: String,
    /// Nombre visible que se recorta y valida.
    pub name: String,
}

/// Patch CAS de caja. El código y los owners son inmutables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CashRegisterUpdateRequest {
    /// Revisión leída por el cliente.
    pub expected_version: u64,
    /// Nombre opcional.
    pub name: Option<String>,
    /// Estado operativo opcional.
    pub status: Option<Status>,
}

/// Apertura de turno; el actor y el scope se derivan del principal verificado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCashShiftRequest {
    /// Caja dentro de la sucursal scoped.
    pub cash_register_id: EntityId,
    /// Declaración inicial con escala decimal máxima cuatro.
    pub opening_amount: String,
}

/// Cierre no idempotente de turno; el actor se deriva del principal verificado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseCashShiftRequest {
    /// Revisión leída antes de intentar el cierre.
    pub expected_version: u64,
    /// Declaración final con escala decimal máxima cuatro.
    pub closing_declared_amount: String,
}

/// Página determinista de cajas o turnos.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CashPage {
    /// Tamaño de página, de 1 a [`MAX_CASH_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento desde el inicio.
    pub offset: u32,
}

/// Casos de uso candidatos de cajas y turnos sobre SQLite real.
pub struct SqliteCashApplication<S> {
    unit_of_work: SqliteUnitOfWork,
    open_sales: S,
}

impl<S: OpenSalesForCashShift> SqliteCashApplication<S> {
    /// Ensambla los casos de uso con un guard de ventas explícito.
    pub fn new(unit_of_work: SqliteUnitOfWork, open_sales: S) -> Self {
        Self {
            unit_of_work,
            open_sales,
        }
    }

    /// Crea una caja activa y audita `cash-register.created` dentro del commit.
    pub async fn create_cash_register<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewCashRegisterRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashRegister, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterCreate, false)?;
        let code =
            CatalogCode::new(&request.code).map_err(|_| ApplicationError::InvalidCashInput)?;
        let name =
            DisplayName::new(&request.name).map_err(|_| ApplicationError::InvalidCashInput)?;
        let register_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let branch = transaction
                .get_branch(&organization_id, &branch_id)
                .await
                .map_err(transaction_error)?
                .ok_or(ApplicationError::BranchNotFound)?;
            if transaction
                .cash_register_code_exists(&organization_id, &branch_id, &code)
                .await
                .map_err(cash_storage_error)?
            {
                return Err(ApplicationError::CashRegisterCodeAlreadyExists);
            }
            let register = CashRegister::create(NewCashRegister {
                id: register_id,
                organization_id: organization_id.clone(),
                branch_id: branch.id().clone(),
                code,
                name,
            });
            transaction
                .insert_cash_register(&register, occurred_at.as_str())
                .await
                .map_err(cash_storage_error)?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "cash-register.created",
                "cash_register",
                register.id().clone(),
                json!({"branchId": branch_id.as_str(), "code": register.code().as_str()}),
                occurred_at,
            )
            .await?;
            Ok(register)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Lee una caja scoped por organización y sucursal.
    pub async fn get_cash_register(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
    ) -> Result<CashRegister, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterRead, false)?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let result = transaction
            .get_cash_register(&organization_id, &branch_id, cash_register_id)
            .await
            .map_err(cash_storage_error)?
            .ok_or(ApplicationError::CashRegisterNotFound);
        finish(transaction, result).await
    }

    /// Lista cajas de la sucursal del principal con límites explícitos.
    pub async fn list_cash_registers(
        &self,
        principal: &VerifiedPrincipal,
        status: Option<Status>,
        page: CashPage,
    ) -> Result<Vec<CashRegister>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterRead, false)?;
        if page.limit == 0 || page.limit > MAX_CASH_PAGE_SIZE {
            return Err(ApplicationError::InvalidCashInput);
        }
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let result = transaction
            .list_cash_registers(
                &organization_id,
                &branch_id,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await
            .map_err(cash_storage_error);
        finish(transaction, result).await
    }

    /// Actualiza nombre o estado por CAS, sin auditar no-ops.
    pub async fn update_cash_register<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: &EntityId,
        request: CashRegisterUpdateRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashRegister, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashRegisterUpdate, false)?;
        let event_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let mut register = transaction
                .get_cash_register(&organization_id, &branch_id, cash_register_id)
                .await
                .map_err(cash_storage_error)?
                .ok_or(ApplicationError::CashRegisterNotFound)?;
            let before = register.clone();
            let previous_revision = register.revision();
            let name = request
                .name
                .as_deref()
                .map(DisplayName::new)
                .transpose()
                .map_err(|_| ApplicationError::InvalidCashInput)?;
            if !register
                .patch(CashRegisterPatch {
                    expected_version: request.expected_version,
                    name,
                    status: request.status,
                })
                .map_err(cash_domain_error)?
            {
                return Ok(register);
            }
            if transaction
                .update_cash_register_cas(previous_revision, &register, occurred_at.as_str())
                .await
                .map_err(cash_storage_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            append_audit_with_id(
                &mut transaction,
                event_id,
                principal,
                "cash-register.updated",
                "cash_register",
                register.id().clone(),
                json!({"changedFields": cash_register_changed_fields(&before, &register)}),
                occurred_at,
            )
            .await?;
            Ok(register)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Abre un turno tras comprobar branch/caja activas; el índice parcial decide carreras.
    pub async fn open_cash_shift<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: OpenCashShiftRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashShift, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftOpen, false)?;
        let opening_amount = UnitPrice::parse(&request.opening_amount)
            .map_err(|_| ApplicationError::InvalidCashInput)?;
        let shift_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            let branch = transaction
                .get_branch(&organization_id, &branch_id)
                .await
                .map_err(transaction_error)?
                .ok_or(ApplicationError::BranchNotFound)?;
            let register = transaction
                .get_cash_register(&organization_id, &branch_id, &request.cash_register_id)
                .await
                .map_err(cash_storage_error)?
                .ok_or(ApplicationError::CashRegisterNotFound)?;
            let has_open_shift = transaction
                .cash_register_has_open_shift(
                    &organization_id,
                    &branch_id,
                    &request.cash_register_id,
                )
                .await
                .map_err(cash_storage_error)?;
            ensure_open_allowed(branch.status() == Status::Active, &register, has_open_shift)
                .map_err(cash_domain_error)?;
            let opened_at = transaction
                .candidate_utc_seconds(occurred_at)
                .await
                .map_err(cash_storage_error)?;
            let shift = CashShift::open(NewCashShift {
                id: shift_id,
                register,
                opened_by_user_id: principal.user_id().clone(),
                opening_amount,
                opened_at,
            })
            .map_err(cash_domain_error)?;
            transaction
                .insert_cash_shift(&shift, occurred_at.as_str())
                .await
                .map_err(cash_storage_error)?;
            append_audit(
                &mut transaction,
                identities,
                principal,
                "cash-shift.opened",
                "cash_shift",
                shift.id().clone(),
                json!({
                    "branchId": branch_id.as_str(),
                    "cashRegisterId": request.cash_register_id.as_str(),
                    "openingAmount": shift.opening_amount().to_string()
                }),
                occurred_at,
            )
            .await?;
            Ok(shift)
        }
        .await;
        finish(transaction, operation).await
    }

    /// Lee el turno abierto de una caja scoped.
    pub async fn get_open_cash_shift(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
    ) -> Result<Option<CashShift>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftRead, false)?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let result = transaction
            .get_open_cash_shift(&organization_id, &branch_id, cash_register_id)
            .await
            .map_err(cash_storage_error);
        finish(transaction, result).await
    }

    /// Lee un turno histórico scoped.
    pub async fn get_cash_shift(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
    ) -> Result<CashShift, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftRead, false)?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let result = transaction
            .get_cash_shift(
                &organization_id,
                &branch_id,
                cash_register_id,
                cash_shift_id,
            )
            .await
            .map_err(cash_storage_error)?
            .ok_or(ApplicationError::CashShiftNotFound);
        finish(transaction, result).await
    }

    /// Lista el historial de turnos scoped.
    pub async fn list_cash_shifts(
        &self,
        principal: &VerifiedPrincipal,
        cash_register_id: &EntityId,
        status: Option<CashShiftStatus>,
        page: CashPage,
    ) -> Result<Vec<CashShift>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftRead, false)?;
        if page.limit == 0 || page.limit > MAX_CASH_PAGE_SIZE {
            return Err(ApplicationError::InvalidCashInput);
        }
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let result = transaction
            .list_cash_shifts(
                &organization_id,
                &branch_id,
                cash_register_id,
                status,
                i64::from(page.limit),
                i64::from(page.offset),
            )
            .await
            .map_err(cash_storage_error);
        finish(transaction, result).await
    }

    /// Cierra un turno OPEN con CAS y guard de ventas dentro de la misma UoW.
    ///
    /// El scope de recuperación es admisible solo aquí: es obligatorio si la
    /// sucursal o caja ya está inactiva y permite finalizar el turno histórico,
    /// pero no abrir ni administrar recursos.
    pub async fn close_cash_shift<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
        request: CloseCashShiftRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<CashShift, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::CashShiftClose, true)?;
        let declared_amount = UnitPrice::parse(&request.closing_declared_amount)
            .map_err(|_| ApplicationError::InvalidCashInput)?;
        let event_id = identities.next_id()?;
        let mut transaction = self.unit_of_work.begin().await.map_err(transaction_error)?;
        let operation = async {
            // El cierre normal opera sobre recursos activos. Si caja o sucursal
            // se desactivaron después de abrir el turno, solo una capacidad de
            // recuperación explícita puede finalizar el hecho histórico.
            let branch = transaction
                .get_branch(&organization_id, &branch_id)
                .await
                .map_err(transaction_error)?
                .ok_or(ApplicationError::BranchNotFound)?;
            let register = transaction
                .get_cash_register(&organization_id, &branch_id, cash_register_id)
                .await
                .map_err(cash_storage_error)?
                .ok_or(ApplicationError::CashRegisterNotFound)?;
            if (branch.status() != Status::Active || !register.is_active())
                && principal.scope().mode() != ScopeMode::Recovery
            {
                return Err(ApplicationError::Forbidden);
            }
            let mut shift = transaction
                .get_cash_shift(
                    &organization_id,
                    &branch_id,
                    cash_register_id,
                    cash_shift_id,
                )
                .await
                .map_err(cash_storage_error)?
                .ok_or(ApplicationError::CashShiftNotFound)?;
            let has_open_sales = self
                .open_sales
                .has_open_sales(&mut transaction, cash_shift_id)
                .await
                .map_err(cash_storage_error)?;
            ensure_close_allowed(&shift, has_open_sales).map_err(cash_domain_error)?;
            let previous_revision = shift.revision();
            let closed_at = transaction
                .candidate_utc_seconds(occurred_at)
                .await
                .map_err(cash_storage_error)?;
            shift
                .close(CloseCashShift {
                    expected_version: request.expected_version,
                    declared_amount,
                    closed_by_user_id: principal.user_id().clone(),
                    closed_at,
                })
                .map_err(cash_domain_error)?;
            if transaction
                .close_cash_shift_cas(previous_revision, &shift, occurred_at.as_str())
                .await
                .map_err(cash_storage_error)?
                != CasOutcome::Updated
            {
                return Err(ApplicationError::ConcurrentModification);
            }
            append_audit_with_id(
                &mut transaction,
                event_id,
                principal,
                "cash-shift.closed",
                "cash_shift",
                shift.id().clone(),
                json!({
                    "cashRegisterId": cash_register_id.as_str(),
                    "declaredCashAmount": declared_amount.to_string()
                }),
                occurred_at,
            )
            .await?;
            Ok(shift)
        }
        .await;
        finish(transaction, operation).await
    }
}

fn cash_domain_error(error: CashError) -> ApplicationError {
    match error {
        CashError::RegisterInactive => ApplicationError::CashRegisterInactive,
        CashError::BranchInactive => ApplicationError::BranchInactive,
        CashError::RegisterAlreadyHasOpenShift => ApplicationError::CashRegisterAlreadyHasOpenShift,
        CashError::ShiftAlreadyClosed => ApplicationError::CashShiftAlreadyClosed,
        CashError::ShiftHasOpenSales => ApplicationError::CashShiftHasOpenSales,
        CashError::ConcurrentModification => ApplicationError::ConcurrentModification,
        CashError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        CashError::InvalidAmount
        | CashError::RevisionOverflow
        | CashError::InvalidPersistedValue => ApplicationError::InvalidCashInput,
    }
}

fn cash_storage_error(error: CashStorageError) -> ApplicationError {
    match error {
        CashStorageError::DuplicateRegisterCode => ApplicationError::CashRegisterCodeAlreadyExists,
        CashStorageError::OpenShiftExists => ApplicationError::CashRegisterAlreadyHasOpenShift,
        CashStorageError::ForeignKey => ApplicationError::OwnershipMismatch,
        CashStorageError::InvalidRow | CashStorageError::Database => ApplicationError::Storage,
    }
}

fn transaction_error(_: crate::OrganizationBranchStorageError) -> ApplicationError {
    ApplicationError::Storage
}

async fn finish<T>(
    transaction: SqliteTransaction,
    operation: Result<T, ApplicationError>,
) -> Result<T, ApplicationError> {
    match operation {
        Ok(value) => {
            transaction.commit().await.map_err(transaction_error)?;
            Ok(value)
        }
        Err(error) => {
            if transaction.rollback().await.is_err() {
                return Err(ApplicationError::Storage);
            }
            Err(error)
        }
    }
}
