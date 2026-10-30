//! Mappers de fila del contexto.

use super::store::*;
use crate::prelude::*;

#[derive(sqlx::FromRow)]
pub(crate) struct CashRegisterRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) branch_id: String,
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) version: i64,
}

impl CashRegisterRow {
    pub(crate) fn into_domain(self) -> Result<CashRegister, CashStorageError> {
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
pub(crate) struct CashShiftRow {
    pub(crate) id: String,
    pub(crate) organization_id: String,
    pub(crate) branch_id: String,
    pub(crate) cash_register_id: String,
    pub(crate) opened_by_user_id: String,
    pub(crate) closed_by_user_id: Option<String>,
    pub(crate) status: String,
    pub(crate) opening_amount: String,
    pub(crate) closing_declared_amount: Option<String>,
    pub(crate) opened_at: i64,
    pub(crate) closed_at: Option<i64>,
    pub(crate) version: i64,
}

impl CashShiftRow {
    pub(crate) fn into_domain(self) -> Result<CashShift, CashStorageError> {
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
