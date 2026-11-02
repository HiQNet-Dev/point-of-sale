//! Handlers IPC: solo traducen entradas y delegan, sin SQL ni reglas empresariales.
use crate::{
    composition::LocalState,
    dto::{
        BootstrapInstallRequestDto, BootstrapStatusDto, BranchAdminDto, BranchDto,
        CatalogItemRequestDto, CategoryDto, CreateBranchRequestDto, CreateMembershipRequestDto,
        CreateProductRequest, CreatedProductDto, InstallationStatusDto, IpcError, LoginRequestDto,
        LogoutAllDto, MembershipDto, NewCategoryRequestDto, NewRoleRequestDto, NewUserRequestDto,
        OrganizationDto, PageRequest, PermissionDto, ProductDto, RoleDto, SelectBranchRequestDto,
        SessionIdentityDto, SetRolePermissionsRequestDto, SetUserRolesRequestDto,
        UpdateBranchRequestDto, UpdateMembershipRequestDto, UpdateOrganizationRequestDto,
        UpdateProductRequestDto, UpdateRoleRequestDto, UpdateUserRequestDto, UserDto,
    },
};
use tauri::State;

/// Consulta sucursales: branch.read, scope organización del principal.
#[tauri::command]
#[specta::specta]
pub async fn lab_list_branches(state: State<'_, LocalState>) -> Result<Vec<BranchDto>, IpcError> {
    state.branches().await
}

/// Configuración de la organización autenticada: organization.read.
#[tauri::command]
#[specta::specta]
pub async fn organization_get(state: State<'_, LocalState>) -> Result<OrganizationDto, IpcError> {
    state.organization_get().await
}

/// Edición de la configuración del negocio: organization.update.
#[tauri::command]
#[specta::specta]
pub async fn organization_update(
    state: State<'_, LocalState>,
    request: UpdateOrganizationRequestDto,
) -> Result<OrganizationDto, IpcError> {
    state.organization_update(request).await
}

/// Sucursales de la organización con su zona horaria efectiva: branch.read.
#[tauri::command]
#[specta::specta]
pub async fn branch_list(state: State<'_, LocalState>) -> Result<Vec<BranchAdminDto>, IpcError> {
    state.branch_list().await
}

/// Sucursal concreta del principal: branch.read.
#[tauri::command]
#[specta::specta]
pub async fn branch_get(
    state: State<'_, LocalState>,
    request: CatalogItemRequestDto,
) -> Result<BranchAdminDto, IpcError> {
    state.branch_get(&request.id).await
}

/// Alta de la sucursal inicial: branch.create con capacidad de edición.
#[tauri::command]
#[specta::specta]
pub async fn branch_create(
    state: State<'_, LocalState>,
    request: CreateBranchRequestDto,
) -> Result<BranchAdminDto, IpcError> {
    state.branch_create(request).await
}

/// Edición de una sucursal existente: branch.update.
#[tauri::command]
#[specta::specta]
pub async fn branch_update(
    state: State<'_, LocalState>,
    request: UpdateBranchRequestDto,
) -> Result<BranchAdminDto, IpcError> {
    state.branch_update(request).await
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

/// Usuarios de la organización autenticada: user.read.
#[tauri::command]
#[specta::specta]
pub async fn user_list(state: State<'_, LocalState>) -> Result<Vec<UserDto>, IpcError> {
    state.user_list().await
}

/// Usuario concreto con sus roles: user.read.
#[tauri::command]
#[specta::specta]
pub async fn user_get(
    state: State<'_, LocalState>,
    request: CatalogItemRequestDto,
) -> Result<UserDto, IpcError> {
    state.user_get(&request.id).await
}

/// Alta de usuario con contraseña inicial: user.create.
#[tauri::command]
#[specta::specta]
pub async fn user_create(
    state: State<'_, LocalState>,
    request: NewUserRequestDto,
) -> Result<UserDto, IpcError> {
    state.user_create(request).await
}

/// Edición de nombre, email o estado: user.update.
#[tauri::command]
#[specta::specta]
pub async fn user_update(
    state: State<'_, LocalState>,
    request: UpdateUserRequestDto,
) -> Result<UserDto, IpcError> {
    state.user_update(request).await
}

/// Reemplazo de roles de organización: role.assign.
#[tauri::command]
#[specta::specta]
pub async fn user_set_roles(
    state: State<'_, LocalState>,
    request: SetUserRolesRequestDto,
) -> Result<UserDto, IpcError> {
    state.user_set_roles(request).await
}

/// Roles de la organización: role.read.
#[tauri::command]
#[specta::specta]
pub async fn role_list(state: State<'_, LocalState>) -> Result<Vec<RoleDto>, IpcError> {
    state.role_list().await
}

/// Rol concreto con sus permisos: role.read.
#[tauri::command]
#[specta::specta]
pub async fn role_get(
    state: State<'_, LocalState>,
    request: CatalogItemRequestDto,
) -> Result<RoleDto, IpcError> {
    state.role_get(&request.id).await
}

/// Alta de rol: role.create y, al conceder permisos, role.permissions.manage.
#[tauri::command]
#[specta::specta]
pub async fn role_create(
    state: State<'_, LocalState>,
    request: NewRoleRequestDto,
) -> Result<RoleDto, IpcError> {
    state.role_create(request).await
}

/// Edición de nombre o estado de un rol: role.update.
#[tauri::command]
#[specta::specta]
pub async fn role_update(
    state: State<'_, LocalState>,
    request: UpdateRoleRequestDto,
) -> Result<RoleDto, IpcError> {
    state.role_update(request).await
}

/// Reemplazo de permisos de un rol: role.permissions.manage.
#[tauri::command]
#[specta::specta]
pub async fn role_set_permissions(
    state: State<'_, LocalState>,
    request: SetRolePermissionsRequestDto,
) -> Result<RoleDto, IpcError> {
    state.role_set_permissions(request).await
}

/// Catálogo de permisos del sistema: permission.read.
#[tauri::command]
#[specta::specta]
pub async fn permission_list(state: State<'_, LocalState>) -> Result<Vec<PermissionDto>, IpcError> {
    state.permission_list().await
}

/// Membresías de sucursal: branch.membership.read.
#[tauri::command]
#[specta::specta]
pub async fn membership_list(state: State<'_, LocalState>) -> Result<Vec<MembershipDto>, IpcError> {
    state.membership_list().await
}

/// Alta de membresía de sucursal: branch.membership.create.
#[tauri::command]
#[specta::specta]
pub async fn membership_create(
    state: State<'_, LocalState>,
    request: CreateMembershipRequestDto,
) -> Result<MembershipDto, IpcError> {
    state.membership_create(request).await
}

/// Edición de estado o roles de una membresía: branch.membership.update y
/// branch.membership.roles.manage.
#[tauri::command]
#[specta::specta]
pub async fn membership_update(
    state: State<'_, LocalState>,
    request: UpdateMembershipRequestDto,
) -> Result<MembershipDto, IpcError> {
    state.membership_update(request).await
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
        organization_get,
        organization_update,
        branch_list,
        branch_get,
        branch_create,
        branch_update,
        user_list,
        user_get,
        user_create,
        user_update,
        user_set_roles,
        role_list,
        role_get,
        role_create,
        role_update,
        role_set_permissions,
        permission_list,
        membership_list,
        membership_create,
        membership_update,
    ])
}
