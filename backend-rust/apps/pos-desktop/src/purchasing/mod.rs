//! Frontera Purchasing preparada, no registrada en el dispatcher del producto.
//!
//! El registro separado solo genera contratos y permite pruebas aisladas del
//! adaptador existente. El runtime no construye este estado ni otorga ACLs.
pub mod commands;
pub mod dto;
mod state;
pub use state::PurchasingState;

/// Registro de las 17 operaciones preparadas, separado del dispatcher activo.
pub fn registry<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new().commands(tauri_specta::collect_commands![
        commands::purchasing_create_supplier,
        commands::purchasing_get_supplier,
        commands::purchasing_list_suppliers,
        commands::purchasing_update_supplier,
        commands::purchasing_create_order,
        commands::purchasing_get_order,
        commands::purchasing_list_orders,
        commands::purchasing_change_supplier,
        commands::purchasing_add_line,
        commands::purchasing_update_line,
        commands::purchasing_remove_line,
        commands::purchasing_issue_order,
        commands::purchasing_cancel_order,
        commands::purchasing_post_receipt,
        commands::purchasing_get_receipt,
        commands::purchasing_list_receipts,
        commands::purchasing_receipt_status,
    ])
}

#[cfg(test)]
mod tests;
