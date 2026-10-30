//! Comandos expuestos a la única ventana local.
//!
//! El modo de la aplicación decide cuáles responden; el registro es único para
//! que el dispatcher real y los bindings generados no puedan divergir.
pub const COMMAND_NAMES: &[&str] = &[
    "lab_list_branches",
    "lab_list_categories",
    "lab_list_products",
    "installation_status",
];

/// Comandos del laboratorio sintético; no existen en el modo instalación.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub const LAB_COMMAND_NAMES: &[&str] = &[
    "lab_list_branches",
    "lab_list_categories",
    "lab_list_products",
];
