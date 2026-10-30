//! Inventario verificable de firmas internas; no equivale a handlers registrados.

use pos_core::organization_branch::Permission;
use serde_json::{Value, json};

const CONTEXTS: [(&str, &str, &str); 5] = [
    (
        "catalog",
        include_str!("../../../crates/pos-application/src/catalog/use_cases.rs"),
        "organization",
    ),
    (
        "cash",
        include_str!("../../../crates/pos-application/src/cash/use_cases.rs"),
        "branch",
    ),
    (
        "sales",
        include_str!("../../../crates/pos-application/src/sales/use_cases.rs"),
        "branch",
    ),
    (
        "inventory",
        include_str!("../../../crates/pos-application/src/inventory/use_cases.rs"),
        "organization; stock also receives branchId",
    ),
    (
        "checkout",
        include_str!("../../../crates/pos-application/src/checkout/use_cases.rs"),
        "branch",
    ),
];

const PERMISSIONS: &[Permission] = &[
    Permission::CategoryCreate,
    Permission::CategoryRead,
    Permission::CategoryUpdate,
    Permission::ProductCreate,
    Permission::ProductRead,
    Permission::ProductUpdate,
    Permission::CashRegisterRead,
    Permission::CashRegisterCreate,
    Permission::CashRegisterUpdate,
    Permission::CashShiftRead,
    Permission::CashShiftOpen,
    Permission::CashShiftClose,
    Permission::SaleRead,
    Permission::SaleCreate,
    Permission::SaleUpdate,
    Permission::SaleCancel,
    Permission::SaleCheckout,
    Permission::InventoryItemRead,
    Permission::InventoryItemWrite,
    Permission::InventoryStockRead,
    Permission::InventoryStockAdjust,
];

/// Exporta firmas y permisos actuales sin inventar DTOs de transporte.
pub fn catalog() -> Result<Value, &'static str> {
    let mut operations = Vec::new();
    for (context, source, scope) in CONTEXTS {
        // Inventario conservador de métodos públicos async. El test obliga a
        // revisar cambios en las firmas, permisos o cantidad de operaciones.
        for method in source.split("pub async fn ").skip(1) {
            let signature = method.split_once(" {").ok_or("firma no reconocida")?.0;
            let name = signature
                .split(['<', '('])
                .next()
                .ok_or("método sin nombre")?;
            let permission = PERMISSIONS
                .iter()
                .find(|permission| method.contains(&format!("Permission::{permission:?}")))
                .and_then(|permission| permission.as_code())
                .ok_or("permiso no inventariado")?;
            operations.push(json!({
                "id": format!("rust.{context}.{name}"),
                "context": context,
                "method": name,
                "status": "IMPLEMENTED_IN_APPLICATION",
                "ipcCommand": null, "httpRoute": null,
                "transportRequest": null, "transportResponse": null,
                "permission": permission, "scope": scope,
                "internalSignature": signature.trim(),
                "source": format!("backend-rust/crates/pos-application/src/{context}/use_cases.rs"),
                "dtoSource": format!("backend-rust/crates/pos-application/src/{context}/dto.rs"),
                "transportBlocker": "No registered transport; physical schema pending explicit approval"
            }));
        }
    }
    Ok(json!({
        "schemaVersion": 1,
        "operations": operations,
        "testOnlyInvocation": {
            "id": crate::contract::LAB_COMMAND, "status": "AVAILABLE_IN_LAB",
            "ipcCommand": null, "httpRoute": null,
            "request": {}, "response": "ScenarioReport",
            "authentication": "Synthetic fixture session; no commercial login",
            "permission": "Application operations enforce DB-authoritative RBAC",
            "test": "frontend/src/shared/rust-local/client.test.ts"
        },
        "notExposed": ["bootstrap", "authentication", "sessions", "organization", "branches", "users", "roles", "rbac", "modifiers", "recipes", "suppliers", "purchase-orders", "purchase-receipts", "audit"],
        "note": "notExposed does not imply absence of domain/application code; no public transport contract is claimed"
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn checked_in_inventory_matches_application() -> Result<(), Box<dyn std::error::Error>> {
        let generated = super::catalog()?;
        let stored: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/03-api/rust-operations.generated.json"
        ))?;
        assert_eq!(generated, stored);
        assert_eq!(generated["operations"].as_array().map(Vec::len), Some(32));
        Ok(())
    }
}
