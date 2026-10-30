//! Consultas SQL del contexto.
//!
//! Solo infraestructura: los casos de uso no conocen estas sentencias.

pub(crate) const INSERT_PAYMENT: &str = "INSERT INTO payment (id, organization_id, branch_id, \
    cash_register_id, cash_shift_id, sale_id, position, method, amount, tendered_amount, \
    change_amount, created_by_user_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const INSERT_INVENTORY_COMPONENT: &str = "INSERT INTO sale_item_inventory_component \
    (id, organization_id, sale_item_id, inventory_item_id, base_unit, quantity_per_unit, \
    quantity_total, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
