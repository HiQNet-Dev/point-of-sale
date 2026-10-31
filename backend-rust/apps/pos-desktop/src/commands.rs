//! Handlers IPC: solo traducen entradas y delegan, sin SQL ni reglas empresariales.
use crate::{
    composition::LocalState,
    dto::{
        BootstrapInstallRequestDto, BootstrapStatusDto, BranchDto, CatalogItemRequestDto,
        CategoryDto, CreateProductRequest, CreatedProductDto, InstallationStatusDto, IpcError,
        LoginRequestDto, LogoutAllDto, NewCategoryRequestDto, PageRequest, ProductDto,
        SelectBranchRequestDto, SessionIdentityDto, UpdateProductRequestDto,
    },
};
use tauri::State;

/// Consulta sucursales: branch.read, scope organización del principal.
#[tauri::command]
#[specta::specta]
pub async fn lab_list_branches(state: State<'_, LocalState>) -> Result<Vec<BranchDto>, IpcError> {
    state.branches().await
}

/// Consulta categorías: category.read, página administrativa de organización.
#[tauri::command]
#[specta::specta]
pub async fn lab_list_categories(
    state: State<'_, LocalState>,
    request: PageRequest,
) -> Result<Vec<CategoryDto>, IpcError> {
    state.categories(request).await
}

/// Consulta productos: product.read; el precio se serializa como texto exacto.
#[tauri::command]
#[specta::specta]
pub async fn lab_list_products(
    state: State<'_, LocalState>,
    request: PageRequest,
) -> Result<Vec<ProductDto>, IpcError> {
    state.products(request).await
}

/// Primera escritura auditada: requiere `product.create` y actor verificado.
#[tauri::command]
#[specta::specta]
pub async fn lab_create_product(
    state: State<'_, LocalState>,
    request: CreateProductRequest,
) -> Result<CreatedProductDto, IpcError> {
    state.create_product(request).await
}

/// Estado de la instalación persistente; no expone datos de negocio ni identidad.
#[tauri::command]
#[specta::specta]
pub fn installation_status(
    state: State<'_, LocalState>,
) -> Result<InstallationStatusDto, IpcError> {
    state.installation_status()
}

/// Estado del asistente de primera instalación.
#[tauri::command]
#[specta::specta]
pub async fn bootstrap_status(
    state: State<'_, LocalState>,
) -> Result<BootstrapStatusDto, IpcError> {
    state.bootstrap_status().await
}

/// Completa la primera instalación consumiendo la autorización nativa.
#[tauri::command]
#[specta::specta]
pub async fn bootstrap_install(
    state: State<'_, LocalState>,
    request: BootstrapInstallRequestDto,
) -> Result<(), IpcError> {
    state.bootstrap_install(request).await
}

/// Lista categorías de la organización autenticada.
#[tauri::command]
#[specta::specta]
pub async fn catalog_list_categories(
    state: State<'_, LocalState>,
    request: PageRequest,
) -> Result<Vec<CategoryDto>, IpcError> {
    state.catalog_list_categories(request).await
}

/// Lista productos de la organización autenticada.
#[tauri::command]
#[specta::specta]
pub async fn catalog_list_products(
    state: State<'_, LocalState>,
    request: PageRequest,
) -> Result<Vec<ProductDto>, IpcError> {
    state.catalog_list_products(request).await
}

/// Consulta un producto de la organización autenticada.
#[tauri::command]
#[specta::specta]
pub async fn catalog_get_product(
    state: State<'_, LocalState>,
    request: CatalogItemRequestDto,
) -> Result<ProductDto, IpcError> {
    state.catalog_get_product(request).await
}

/// Crea una categoría como actor autenticado.
#[tauri::command]
#[specta::specta]
pub async fn catalog_create_category(
    state: State<'_, LocalState>,
    request: NewCategoryRequestDto,
) -> Result<CategoryDto, IpcError> {
    state.catalog_create_category(request).await
}

/// Crea un producto como actor autenticado.
#[tauri::command]
#[specta::specta]
pub async fn catalog_create_product(
    state: State<'_, LocalState>,
    request: CreateProductRequest,
) -> Result<CreatedProductDto, IpcError> {
    state.catalog_create_product(request).await
}

/// Edita un producto con CAS sobre la versión leída.
#[tauri::command]
#[specta::specta]
pub async fn catalog_update_product(
    state: State<'_, LocalState>,
    request: UpdateProductRequestDto,
) -> Result<ProductDto, IpcError> {
    state.catalog_update_product(request).await
}

/// Autentica con email y contraseña; el handle queda en memoria nativa.
#[tauri::command]
#[specta::specta]
pub async fn auth_login(
    state: State<'_, LocalState>,
    request: LoginRequestDto,
) -> Result<SessionIdentityDto, IpcError> {
    state.auth_login(request).await
}

/// Identidad y permisos de la sesión nativa vigente.
#[tauri::command]
#[specta::specta]
pub async fn auth_me(state: State<'_, LocalState>) -> Result<SessionIdentityDto, IpcError> {
    state.auth_me().await
}

/// Revoca la sesión actual.
#[tauri::command]
#[specta::specta]
pub async fn auth_logout(state: State<'_, LocalState>) -> Result<(), IpcError> {
    state.auth_logout().await
}

/// Revoca todas las sesiones del usuario autenticado.
#[tauri::command]
#[specta::specta]
pub async fn auth_logout_all(state: State<'_, LocalState>) -> Result<LogoutAllDto, IpcError> {
    state.auth_logout_all().await
}

/// Selecciona una sucursal activa del tenant y devuelve sus permisos.
#[tauri::command]
#[specta::specta]
pub async fn auth_select_branch(
    state: State<'_, LocalState>,
    request: SelectBranchRequestDto,
) -> Result<SessionIdentityDto, IpcError> {
    state.auth_select_branch(request).await
}

/// Lista las sucursales descubribles por la sesión actual.
#[tauri::command]
#[specta::specta]
pub async fn auth_list_branches(state: State<'_, LocalState>) -> Result<Vec<BranchDto>, IpcError> {
    state.auth_list_branches().await
}

/// Registro único utilizado para el dispatcher real y la generación TypeScript.
pub fn registry<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new().commands(tauri_specta::collect_commands![
        lab_list_branches,
        lab_list_categories,
        lab_list_products,
        lab_create_product,
        installation_status,
        bootstrap_status,
        bootstrap_install,
        catalog_list_categories,
        catalog_list_products,
        catalog_get_product,
        catalog_create_category,
        catalog_create_product,
        catalog_update_product,
        auth_login,
        auth_me,
        auth_logout,
        auth_logout_all,
        auth_select_branch,
        auth_list_branches,
    ])
}
