//! Handlers IPC: solo traducen entradas y delegan, sin SQL ni reglas empresariales.
use crate::{
    composition::LocalState,
    dto::{
        BranchDto, CategoryDto, CreateProductRequest, CreatedProductDto, InstallationStatusDto,
        IpcError, PageRequest, ProductDto,
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

/// Registro único utilizado para el dispatcher real y la generación TypeScript.
pub fn registry<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new().commands(tauri_specta::collect_commands![
        lab_list_branches,
        lab_list_categories,
        lab_list_products,
        lab_create_product,
        installation_status,
    ])
}
