//! Consultas SQL del contexto.
//!
//! Solo infraestructura: los casos de uso no conocen estas sentencias.

pub(crate) const SELECT_SALE: &str = "SELECT id, organization_id, branch_id, cash_register_id, cash_shift_id, \
    created_by_user_id, cancelled_by_user_id, completed_by_user_id, status, total_amount, version, \
    CAST(strftime('%s', cancelled_at) AS INTEGER) AS cancelled_at, \
    CAST(strftime('%s', completed_at) AS INTEGER) AS completed_at \
    FROM sale WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND cash_shift_id = ? AND id = ?";

pub(crate) const LIST_SALES: &str = "SELECT id, organization_id, branch_id, cash_register_id, cash_shift_id, \
    created_by_user_id, cancelled_by_user_id, completed_by_user_id, status, total_amount, version, \
    CAST(strftime('%s', cancelled_at) AS INTEGER) AS cancelled_at, \
    CAST(strftime('%s', completed_at) AS INTEGER) AS completed_at \
    FROM sale WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND cash_shift_id = ? \
    AND (? IS NULL OR status = ?) ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?";

pub(crate) const INSERT_SALE: &str = "INSERT INTO sale (id, organization_id, branch_id, cash_register_id, cash_shift_id, \
    created_by_user_id, cancelled_by_user_id, completed_by_user_id, status, total_amount, version, \
    created_at, updated_at, cancelled_at, completed_at) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, ?, ?, ?, NULL, NULL)";

pub(crate) const UPDATE_SALE_CAS: &str = "UPDATE sale SET cancelled_by_user_id = ?, completed_by_user_id = ?, status = ?, \
    total_amount = ?, version = ?, updated_at = ?, cancelled_at = ?, completed_at = ? \
    WHERE organization_id = ? AND branch_id = ? AND cash_register_id = ? AND cash_shift_id = ? AND id = ? AND version = ?";

pub(crate) const SELECT_SALE_VERSION: &str = "SELECT version FROM sale WHERE organization_id = ? AND branch_id = ? \
    AND cash_register_id = ? AND cash_shift_id = ? AND id = ?";

pub(crate) const SELECT_ITEM: &str = "SELECT id, organization_id, sale_id, product_id, product_sku_snapshot, product_name_snapshot, \
    base_unit_price, modifier_unit_delta, effective_unit_price, quantity, line_total, status, position, \
    removed_by_user_id, CAST(strftime('%s', removed_at) AS INTEGER) AS removed_at \
    FROM sale_item WHERE organization_id = ? AND sale_id = ? AND id = ?";

pub(crate) const LIST_ITEMS: &str = "SELECT id, organization_id, sale_id, product_id, product_sku_snapshot, product_name_snapshot, \
    base_unit_price, modifier_unit_delta, effective_unit_price, quantity, line_total, status, position, \
    removed_by_user_id, CAST(strftime('%s', removed_at) AS INTEGER) AS removed_at \
    FROM sale_item WHERE organization_id = ? AND sale_id = ? ORDER BY position, id";

pub(crate) const NEXT_ITEM_POSITION: &str = "SELECT COALESCE(MAX(position), -1) + 1 FROM sale_item WHERE organization_id = ? AND sale_id = ?";

pub(crate) const INSERT_ITEM: &str = "INSERT INTO sale_item (id, organization_id, sale_id, product_id, product_sku_snapshot, product_name_snapshot, \
    base_unit_price, modifier_unit_delta, effective_unit_price, quantity, line_total, status, position, created_at, updated_at, removed_at, removed_by_user_id) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL)";

pub(crate) const UPDATE_ITEM: &str = "UPDATE sale_item SET product_sku_snapshot = ?, product_name_snapshot = ?, base_unit_price = ?, \
    modifier_unit_delta = ?, effective_unit_price = ?, quantity = ?, line_total = ?, updated_at = ? \
    WHERE organization_id = ? AND sale_id = ? AND id = ? AND status = 'ACTIVE'";

pub(crate) const REMOVE_ITEM: &str = "UPDATE sale_item SET status = 'REMOVED', removed_by_user_id = ?, removed_at = ?, updated_at = ? \
    WHERE organization_id = ? AND sale_id = ? AND id = ? AND status = 'ACTIVE'";

pub(crate) const LIST_ITEM_OPTION_IDS: &str = "SELECT modifier_option_id FROM sale_item_modifier WHERE sale_item_id = ? ORDER BY modifier_option_id";

pub(crate) const DELETE_ITEM_MODIFIERS: &str =
    "DELETE FROM sale_item_modifier WHERE sale_item_id = ?";

pub(crate) const INSERT_ITEM_MODIFIER: &str = "INSERT INTO sale_item_modifier (id, organization_id, sale_item_id, modifier_group_id, modifier_option_id, \
    group_code_snapshot, group_name_snapshot, option_code_snapshot, option_name_snapshot, price_delta_snapshot, display_order, created_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const SELECT_MODIFIER_CONFIGURATION: &str = "SELECT pmg.modifier_group_id, \
    mg.code AS group_code, mg.name AS group_name, mg.status AS group_status, mg.min_selections, mg.max_selections, \
    mo.id AS option_id, mo.code AS option_code, mo.name AS option_name, mo.status AS option_status, \
    mo.price_delta, mo.display_order AS option_display_order \
    FROM product_modifier_group pmg \
    JOIN modifier_group mg ON mg.id = pmg.modifier_group_id AND mg.organization_id = pmg.organization_id \
    LEFT JOIN modifier_option mo ON mo.modifier_group_id = mg.id AND mo.organization_id = mg.organization_id \
    WHERE pmg.organization_id = ? AND pmg.product_id = ? \
    ORDER BY pmg.display_order, pmg.modifier_group_id, mo.display_order, mo.id";
