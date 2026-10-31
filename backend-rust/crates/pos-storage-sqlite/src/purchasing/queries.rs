//! SQL del adaptador de compras.
//!
//! Todas las sentencias son estáticas y parametrizadas. El scope de organización
//! y sucursal viaja siempre como parámetro; ninguna consulta filtra en memoria.
//! Las cantidades se suman en Rust con aritmética exacta; SQLite nunca las
//! convierte a coma flotante.

pub(crate) const SELECT_ORGANIZATION_CURRENCY: &str =
    "SELECT currency FROM organization WHERE id = ?";

pub(crate) const SELECT_SUPPLIER: &str = "SELECT id, organization_id, code, name, legal_name, \
    tax_id, contact_name, email, phone, address, status, version FROM supplier \
    WHERE organization_id = ? AND id = ?";

pub(crate) const SELECT_SUPPLIER_BY_CODE: &str =
    "SELECT 1 FROM supplier WHERE organization_id = ? AND code = ?";

pub(crate) const SELECT_SUPPLIERS: &str = "SELECT id, organization_id, code, name, legal_name, \
    tax_id, contact_name, email, phone, address, status, version FROM supplier \
    WHERE organization_id = ? AND (? IS NULL OR status = ?) \
    ORDER BY name, code, id LIMIT ? OFFSET ?";

pub(crate) const INSERT_SUPPLIER: &str = "INSERT INTO supplier (id, organization_id, code, name, \
    legal_name, tax_id, contact_name, email, phone, address, status, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const UPDATE_SUPPLIER_CAS: &str = "UPDATE supplier SET name = ?, legal_name = ?, \
    tax_id = ?, contact_name = ?, email = ?, phone = ?, address = ?, status = ?, version = ?, \
    updated_at = ? WHERE organization_id = ? AND id = ? AND version = ?";

pub(crate) const SELECT_INVENTORY_ITEM: &str = "SELECT id, organization_id, code, name, \
    description, base_unit, fractional_unit_allowed, unit_locked, status, version \
    FROM inventory_item WHERE organization_id = ? AND id = ?";

pub(crate) const SELECT_PURCHASE_ORDER: &str = "SELECT id, organization_id, branch_id, supplier_id, \
    code, status, currency, supplier_code_snapshot, supplier_name_snapshot, created_by_user_id, \
    issued_by_user_id, cancelled_by_user_id, issued_at, cancelled_at, version FROM purchase_order \
    WHERE organization_id = ? AND branch_id = ? AND id = ?";

pub(crate) const SELECT_PURCHASE_ORDERS: &str = "SELECT id, organization_id, branch_id, supplier_id, \
    code, status, currency, supplier_code_snapshot, supplier_name_snapshot, created_by_user_id, \
    issued_by_user_id, cancelled_by_user_id, issued_at, cancelled_at, version FROM purchase_order \
    WHERE organization_id = ? AND branch_id = ? AND (? IS NULL OR status = ?) \
    ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?";

pub(crate) const SELECT_PURCHASE_ORDER_BY_CODE: &str =
    "SELECT 1 FROM purchase_order WHERE organization_id = ? AND code = ?";

pub(crate) const INSERT_PURCHASE_ORDER: &str = "INSERT INTO purchase_order (id, organization_id, \
    branch_id, supplier_id, code, status, currency, supplier_code_snapshot, supplier_name_snapshot, \
    created_by_user_id, issued_by_user_id, cancelled_by_user_id, issued_at, cancelled_at, version, \
    created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const UPDATE_PURCHASE_ORDER_CAS: &str = "UPDATE purchase_order SET supplier_id = ?, \
    status = ?, supplier_code_snapshot = ?, supplier_name_snapshot = ?, issued_by_user_id = ?, \
    cancelled_by_user_id = ?, issued_at = ?, cancelled_at = ?, version = ?, updated_at = ? \
    WHERE organization_id = ? AND branch_id = ? AND id = ? AND version = ?";

pub(crate) const SELECT_ORDER_LINES: &str = "SELECT id, organization_id, purchase_order_id, \
    inventory_item_id, line_number, item_code_snapshot, item_name_snapshot, unit_snapshot, \
    quantity, unit_cost FROM purchase_order_line \
    WHERE organization_id = ? AND purchase_order_id = ? ORDER BY line_number, id";

pub(crate) const SELECT_NEXT_LINE_NUMBER: &str = "SELECT COALESCE(MAX(line_number), 0) + 1 \
    FROM purchase_order_line WHERE organization_id = ? AND purchase_order_id = ?";

pub(crate) const INSERT_ORDER_LINE: &str = "INSERT INTO purchase_order_line (id, organization_id, \
    purchase_order_id, inventory_item_id, line_number, item_code_snapshot, item_name_snapshot, \
    unit_snapshot, quantity, unit_cost, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const UPDATE_ORDER_LINE: &str = "UPDATE purchase_order_line SET quantity = ?, \
    unit_cost = ?, updated_at = ? WHERE organization_id = ? AND purchase_order_id = ? AND id = ?";

pub(crate) const DELETE_ORDER_LINE: &str = "DELETE FROM purchase_order_line \
    WHERE organization_id = ? AND purchase_order_id = ? AND id = ?";

/// Cantidades ya recibidas de las partidas de una orden, en texto exacto.
pub(crate) const SELECT_RECEIVED_LINES: &str = "SELECT rl.purchase_order_line_id AS line_id, \
    rl.quantity AS quantity FROM purchase_receipt_line rl \
    JOIN purchase_order_line l ON l.id = rl.purchase_order_line_id \
        AND l.organization_id = rl.organization_id \
    WHERE l.organization_id = ? AND l.purchase_order_id = ?";

pub(crate) const SELECT_RECEIPT_BY_KEY: &str = "SELECT id, organization_id, branch_id, \
    purchase_order_id, idempotency_key, actor_user_id, received_at FROM purchase_receipt \
    WHERE organization_id = ? AND idempotency_key = ?";

pub(crate) const SELECT_RECEIPT: &str = "SELECT id, organization_id, branch_id, purchase_order_id, \
    idempotency_key, actor_user_id, received_at FROM purchase_receipt \
    WHERE organization_id = ? AND branch_id = ? AND id = ?";

pub(crate) const SELECT_RECEIPTS: &str = "SELECT id, organization_id, branch_id, purchase_order_id, \
    idempotency_key, actor_user_id, received_at FROM purchase_receipt \
    WHERE organization_id = ? AND branch_id = ? \
        AND (? IS NULL OR purchase_order_id = ?) \
    ORDER BY received_at DESC, id DESC LIMIT ? OFFSET ?";

pub(crate) const SELECT_RECEIPT_LINES: &str = "SELECT id, organization_id, purchase_receipt_id, \
    purchase_order_line_id, inventory_item_id, quantity FROM purchase_receipt_line \
    WHERE organization_id = ? AND purchase_receipt_id = ? ORDER BY id";

pub(crate) const INSERT_RECEIPT: &str = "INSERT INTO purchase_receipt (id, organization_id, \
    branch_id, purchase_order_id, idempotency_key, actor_user_id, received_at, created_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const INSERT_RECEIPT_LINE: &str = "INSERT INTO purchase_receipt_line (id, \
    organization_id, purchase_receipt_id, purchase_order_line_id, inventory_item_id, quantity, \
    created_at) VALUES (?, ?, ?, ?, ?, ?, ?)";

pub(crate) const SELECT_BALANCE: &str = "SELECT quantity_on_hand FROM inventory_balance \
    WHERE organization_id = ? AND branch_id = ? AND inventory_item_id = ?";

pub(crate) const UPSERT_BALANCE: &str = "INSERT INTO inventory_balance (id, organization_id, \
    branch_id, inventory_item_id, quantity_on_hand, version, created_at, updated_at) \
    VALUES (?, ?, ?, ?, ?, 0, ?, ?) \
    ON CONFLICT (branch_id, inventory_item_id) DO UPDATE SET \
    quantity_on_hand = excluded.quantity_on_hand, version = inventory_balance.version + 1, \
    updated_at = excluded.updated_at";

pub(crate) const INSERT_LEDGER: &str = "INSERT INTO inventory_ledger (id, organization_id, \
    branch_id, inventory_item_id, movement_type, quantity_delta, resulting_quantity, reason, \
    actor_user_id, occurred_at, sale_id, purchase_receipt_line_id) \
    VALUES (?, ?, ?, ?, 'PURCHASE_RECEIPT', ?, ?, NULL, ?, ?, NULL, ?)";
