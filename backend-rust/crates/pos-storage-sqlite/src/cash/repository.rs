//! Implementación del repositorio sobre la transacción SQLite.

use super::mapper::*;
use super::queries::*;
use super::store::*;
use crate::prelude::*;

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

impl CashTransaction for SqliteTransaction {
    fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Branch>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        async move {
            self.get_branch(&organization_id, &branch_id)
                .await
                .map_err(transaction_error)
        }
    }

    fn get_cash_register(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashRegister>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let cash_register_id = cash_register_id.clone();
        async move {
            self.get_cash_register(&organization_id, &branch_id, &cash_register_id)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn list_cash_registers(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        status: Option<Status>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<CashRegister>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        async move {
            self.list_cash_registers(&organization_id, &branch_id, status, limit, offset)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn cash_register_code_exists(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        code: &CatalogCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let code = code.clone();
        async move {
            self.cash_register_code_exists(&organization_id, &branch_id, &code)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn insert_cash_register(
        &mut self,
        register: &CashRegister,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let register = register.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.insert_cash_register(&register, &occurred_at)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn update_cash_register_cas(
        &mut self,
        previous_revision: u64,
        register: &CashRegister,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let register = register.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.update_cash_register_cas(previous_revision, &register, &occurred_at)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn get_open_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashShift>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let cash_register_id = cash_register_id.clone();
        async move {
            self.get_open_cash_shift(&organization_id, &branch_id, &cash_register_id)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn get_cash_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        cash_shift_id: &EntityId,
    ) -> impl Future<Output = Result<Option<CashShift>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let cash_register_id = cash_register_id.clone();
        let cash_shift_id = cash_shift_id.clone();
        async move {
            self.get_cash_shift(
                &organization_id,
                &branch_id,
                &cash_register_id,
                &cash_shift_id,
            )
            .await
            .map_err(cash_storage_error)
        }
    }

    fn list_cash_shifts(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
        status: Option<CashShiftStatus>,
        limit: i64,
        offset: i64,
    ) -> impl Future<Output = Result<Vec<CashShift>, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let cash_register_id = cash_register_id.clone();
        async move {
            self.list_cash_shifts(
                &organization_id,
                &branch_id,
                &cash_register_id,
                status,
                limit,
                offset,
            )
            .await
            .map_err(cash_storage_error)
        }
    }

    fn cash_register_has_open_shift(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        cash_register_id: &EntityId,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send {
        let organization_id = organization_id.clone();
        let branch_id = branch_id.clone();
        let cash_register_id = cash_register_id.clone();
        async move {
            self.cash_register_has_open_shift(&organization_id, &branch_id, &cash_register_id)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn candidate_utc_seconds(
        &mut self,
        occurred_at: &AuditOccurredAt,
    ) -> impl Future<Output = Result<UtcSeconds, ApplicationError>> + Send {
        let occurred_at = occurred_at.clone();
        async move {
            self.candidate_utc_seconds(&occurred_at)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn insert_cash_shift(
        &mut self,
        shift: &CashShift,
        opened_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send {
        let shift = shift.clone();
        let opened_at = opened_at.to_owned();
        async move {
            self.insert_cash_shift(&shift, &opened_at)
                .await
                .map_err(cash_storage_error)
        }
    }

    fn close_cash_shift_cas(
        &mut self,
        previous_revision: u64,
        shift: &CashShift,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send {
        let shift = shift.clone();
        let occurred_at = occurred_at.to_owned();
        async move {
            self.close_cash_shift_cas(previous_revision, &shift, &occurred_at)
                .await
                .map_err(cash_storage_error)
        }
    }
}
