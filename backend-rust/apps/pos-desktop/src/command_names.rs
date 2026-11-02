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
    "catalog_list_categories",
    "catalog_list_products",
    "catalog_get_product",
    "catalog_create_category",
    "catalog_create_product",
    "catalog_update_product",
    "auth_login",
    "auth_me",
    "auth_logout",
    "auth_logout_all",
    "auth_select_branch",
    "auth_list_branches",
    "organization_get",
    "organization_update",
    "branch_list",
    "branch_get",
    "branch_create",
    "branch_update",
    "user_list",
    "user_get",
    "user_create",
    "user_update",
    "user_set_roles",
    "role_list",
    "role_get",
    "role_create",
    "role_update",
    "role_set_permissions",
    "permission_list",
    "membership_list",
    "membership_create",
    "membership_update",
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

/// Comandos de catálogo comercial; requieren sesión y no existen en laboratorio.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub const CATALOG_COMMAND_NAMES: &[&str] = &[
    "catalog_list_categories",
    "catalog_list_products",
    "catalog_get_product",
    "catalog_create_category",
    "catalog_create_product",
    "catalog_update_product",
];

/// Comandos de configuración del negocio; requieren sesión y permisos propios.
///
/// `branch_create` existe únicamente para configurar la sucursal inicial: la
/// capacidad de Free Local se aplica en la capa de aplicación y rechaza la
/// segunda alta.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub const ORGANIZATION_COMMAND_NAMES: &[&str] = &[
    "organization_get",
    "organization_update",
    "branch_list",
    "branch_get",
    "branch_create",
    "branch_update",
];

/// Comandos de administración de identidad; requieren sesión y permiso propio.
///
/// Ninguno acepta organización, actor ni permisos desde el renderer: la
/// autorización se revalida en SQLite en cada llamada.
#[allow(dead_code)] // Consumido por las pruebas del dispatcher.
pub const ADMINISTRATION_COMMAND_NAMES: &[&str] = &[
    "user_list",
    "user_get",
    "user_create",
    "user_update",
    "user_set_roles",
    "role_list",
    "role_get",
    "role_create",
    "role_update",
    "role_set_permissions",
    "permission_list",
    "membership_list",
    "membership_create",
    "membership_update",
];
