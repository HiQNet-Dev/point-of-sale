//! Handlers IPC: solo traducen entradas y delegan, sin SQL ni reglas empresariales.
use crate::{composition::DesktopState, dto::*};
use tauri::State;

/// Consulta sucursales: branch.read, scope organización del principal.
#[tauri::command]
#[specta::specta]
pub async fn lab_list_branches(state: State<'_, DesktopState>) -> Result<Vec<BranchDto>, IpcError> {
    state.branches().await
}

/// Consulta categorías: category.read, página administrativa de organización.
#[tauri::command]
#[specta::specta]
pub async fn lab_list_categories(
    state: State<'_, DesktopState>,
    request: PageRequest,
) -> Result<Vec<CategoryDto>, IpcError> {
    state.categories(request).await
}

/// Consulta productos: product.read; el precio se serializa como texto exacto.
#[tauri::command]
#[specta::specta]
pub async fn lab_list_products(
    state: State<'_, DesktopState>,
    request: PageRequest,
) -> Result<Vec<ProductDto>, IpcError> {
    state.products(request).await
}

/// Registro único utilizado para el dispatcher real y la generación TypeScript.
pub fn registry<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new().commands(tauri_specta::collect_commands![
        lab_list_branches,
        lab_list_categories,
        lab_list_products,
    ])
}
