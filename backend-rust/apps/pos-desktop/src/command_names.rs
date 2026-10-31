//! Comandos expuestos a la única ventana local.
//!
//! El modo de la aplicación decide cuáles responden; el registro es único para
//! que el dispatcher real y los bindings generados no puedan divergir.
pub const COMMAND_NAMES: &[&str] = &[
    "lab_list_branches",
    "lab_list_categories",
    "lab_list_products",
    "lab_create_product",
    "installation_status",
    "bootstrap_status",
    "bootstrap_install",
    "auth_login",
    "auth_me",
    "auth_logout",
    "auth_logout_all",
    "auth_select_branch",
    "auth_list_branches",
];

/// Comandos del laboratorio sintético; no existen en el modo instalación.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub const LAB_COMMAND_NAMES: &[&str] = &[
    "lab_list_branches",
    "lab_list_categories",
    "lab_list_products",
    "lab_create_product",
];

/// Comandos de autenticación; no existen en el laboratorio sintético.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub const AUTH_COMMAND_NAMES: &[&str] = &[
    "auth_login",
    "auth_me",
    "auth_logout",
    "auth_logout_all",
    "auth_select_branch",
    "auth_list_branches",
];

/// Comandos de primera instalación; no existen en el laboratorio sintético.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub const BOOTSTRAP_COMMAND_NAMES: &[&str] = &["bootstrap_status", "bootstrap_install"];
