//! Consultas SQL del contexto.
//!
//! Solo infraestructura: los casos de uso no conocen estas sentencias.

pub(crate) const SELECT_REGISTER: &str = "SELECT id, organization_id, branch_id, code, name, status, version \
    FROM cash_register WHERE organization_id = ? AND branch_id = ? AND id = ?";

pub(crate) const LIST_REGISTERS: &str = "SELECT id, organization_id, branch_id, code, name, status, version \
    FROM cash_register WHERE organization_id = ? AND branch_id = ? AND (? IS NULL OR status = ?) \
    ORDER BY name, id LIMIT ? OFFSET ?";

pub(crate) const REGISTER_CODE_EXISTS: &str = "SELECT EXISTS(SELECT 1 FROM cash_register \
    WHERE organization_id = ? AND branch_id = ? AND code = ?)";

pub(crate) const INSERT_REGISTER: &str = "INSERT INTO cash_register \
    (id, organization_id, branch_id, code, name, status, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const UPDATE_REGISTER_CAS: &str = "UPDATE cash_register SET name = ?, status = ?, version = ?, updated_at = ? \
    WHERE organization_id = ? AND branch_id = ? AND id = ? AND version = ?";

pub(crate) const SELECT_REGISTER_VERSION: &str = "SELECT version FROM cash_register \
    WHERE organization_id = ? AND branch_id = ? AND id = ?";

pub(crate) const SELECT_SHIFT: &str = "SELECT id, organization_id, branch_id, cash_register_id, opened_by_user_id, \
    closed_by_user_id, status, opening_amount, closing_declared_amount, \
    CAST(strftime('%s', opened_at) AS INTEGER) AS opened_at, \
    CAST(strftime('%s', closed_at) AS INTEGER) AS closed_at, version \
    FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND id = ?";

pub(crate) const SELECT_OPEN_SHIFT: &str = "SELECT id, organization_id, branch_id, cash_register_id, opened_by_user_id, \
    closed_by_user_id, status, opening_amount, closing_declared_amount, \
    CAST(strftime('%s', opened_at) AS INTEGER) AS opened_at, \
    CAST(strftime('%s', closed_at) AS INTEGER) AS closed_at, version \
    FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? \
    AND status = 'OPEN' ORDER BY opened_at DESC, id DESC LIMIT 1";

pub(crate) const LIST_SHIFTS: &str = "SELECT id, organization_id, branch_id, cash_register_id, opened_by_user_id, \
    closed_by_user_id, status, opening_amount, closing_declared_amount, \
    CAST(strftime('%s', opened_at) AS INTEGER) AS opened_at, \
    CAST(strftime('%s', closed_at) AS INTEGER) AS closed_at, version \
    FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? \
    AND (? IS NULL OR status = ?) ORDER BY opened_at DESC, id DESC LIMIT ? OFFSET ?";

pub(crate) const OPEN_SHIFT_EXISTS: &str = "SELECT EXISTS(SELECT 1 FROM cash_shift \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND status = 'OPEN')";

pub(crate) const INSERT_SHIFT: &str = "INSERT INTO cash_shift \
    (id, organization_id, branch_id, cash_register_id, opened_by_user_id, closed_by_user_id, \
     status, opening_amount, closing_declared_amount, opened_at, closed_at, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, NULL, ?, ?, NULL, ?, NULL, ?, ?, ?)";

pub(crate) const UPDATE_SHIFT_CAS: &str = "UPDATE cash_shift SET closed_by_user_id = ?, status = ?, \
    closing_declared_amount = ?, closed_at = ?, version = ?, updated_at = ? \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND id = ? AND version = ? \
    AND status = 'OPEN'";

pub(crate) const SELECT_SHIFT_VERSION: &str = "SELECT version FROM cash_shift \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND id = ?";
