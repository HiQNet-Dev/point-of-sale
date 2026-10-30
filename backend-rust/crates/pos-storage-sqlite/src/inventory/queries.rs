//! Consultas SQL del contexto.
//!
//! Solo infraestructura: los casos de uso no conocen estas sentencias.

pub(crate) const SELECT_ITEM: &str = "SELECT id, organization_id, code, name, description, base_unit, \
    fractional_unit_allowed, unit_locked, status, version \
    FROM inventory_item WHERE organization_id = ? AND id = ?";

pub(crate) const LIST_ITEMS: &str = "SELECT id, organization_id, code, name, description, base_unit, \
    fractional_unit_allowed, unit_locked, status, version \
    FROM inventory_item WHERE organization_id = ? ORDER BY code, id LIMIT ? OFFSET ?";

pub(crate) const INSERT_ITEM: &str = "INSERT INTO inventory_item (id, organization_id, code, name, description, \
    base_unit, fractional_unit_allowed, unit_locked, status, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?)";

pub(crate) const LOCK_UNIT: &str = "UPDATE inventory_item SET unit_locked = 1, updated_at = ? \
    WHERE organization_id = ? AND id = ? AND unit_locked = 0";

pub(crate) const SELECT_BALANCE: &str = "SELECT id, organization_id, branch_id, inventory_item_id, \
    quantity_on_hand, version FROM inventory_balance \
    WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ?";

pub(crate) const INSERT_BALANCE: &str = "INSERT INTO inventory_balance (id, organization_id, branch_id, \
    inventory_item_id, quantity_on_hand, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const UPDATE_BALANCE_CAS: &str = "UPDATE inventory_balance SET quantity_on_hand = ?, version = ?, \
    updated_at = ? WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ? AND version = ?";

pub(crate) const BALANCE_EXISTS: &str = "SELECT EXISTS(SELECT 1 FROM inventory_balance \
    WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ?)";

pub(crate) const LEDGER_COUNTS: &str = "SELECT \
    EXISTS(SELECT 1 FROM inventory_ledger WHERE organization_id = ? AND branch_id = ? \
        AND inventory_item_id = ? AND movement_type = 'OPENING') AS has_opening, \
    EXISTS(SELECT 1 FROM inventory_ledger WHERE organization_id = ? AND branch_id = ? \
        AND inventory_item_id = ?) AS has_any";

pub(crate) const INSERT_MOVEMENT: &str = "INSERT INTO inventory_ledger (id, organization_id, branch_id, \
    inventory_item_id, movement_type, quantity_delta, resulting_quantity, reason, actor_user_id, \
    occurred_at, sale_id, purchase_receipt_line_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const SELECT_MOVEMENTS: &str = "SELECT id, organization_id, branch_id, inventory_item_id, \
    movement_type, quantity_delta, resulting_quantity, reason, actor_user_id, occurred_at, \
    sale_id, purchase_receipt_line_id FROM inventory_ledger \
    WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ? \
    ORDER BY occurred_at DESC, id DESC LIMIT ? OFFSET ?";

pub(crate) const SELECT_ACTIVE_RECIPE: &str = "SELECT r.status AS recipe_status, \
    c.inventory_item_id AS inventory_item_id, c.quantity_per_product AS quantity_per_product, \
    i.base_unit AS base_unit, i.fractional_unit_allowed AS fractional_unit_allowed, \
    (i.status = 'ACTIVE') AS active \
    FROM product_recipe r \
    LEFT JOIN recipe_component c ON c.recipe_id = r.id AND c.organization_id = r.organization_id \
    LEFT JOIN inventory_item i ON i.id = c.inventory_item_id AND i.organization_id = c.organization_id \
    WHERE r.organization_id = ? AND r.product_id = ? \
    ORDER BY c.position, c.inventory_item_id";

pub(crate) const SELECT_ITEM_EFFECTS: &str = "SELECT e.inventory_item_id AS inventory_item_id, \
    i.base_unit AS base_unit, i.fractional_unit_allowed AS fractional_unit_allowed, \
    (i.status = 'ACTIVE') AS active, e.quantity_delta AS quantity_delta \
    FROM sale_item_modifier sim \
    JOIN modifier_inventory_effect e ON e.modifier_option_id = sim.modifier_option_id \
        AND e.organization_id = sim.organization_id \
    JOIN inventory_item i ON i.id = e.inventory_item_id AND i.organization_id = e.organization_id \
    WHERE sim.organization_id = ? AND sim.sale_item_id = ? \
    ORDER BY e.inventory_item_id";
