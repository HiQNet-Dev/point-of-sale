//! Contratos de la administración de identidad.
//!
//! Los campos llegan sin validar: el caso de uso aplica los mismos value objects
//! que cualquier otra escritura. Un campo ausente significa «conservar»; nunca se
//! restablece un valor por defecto.
//!
//! # Contrato de entrada
//!
//! Ninguna petición puede expresar identidad de organización, actor, permisos ni
//! estado del propio llamador. La organización sale del
//! [`pos_core::identity::VerifiedPrincipal`] y los permisos se revalidan en
//! SQLite en cada operación.

use std::collections::BTreeSet;

use pos_core::identity::{BranchMembership, PosUser, Role, UtcSeconds};

/// Longitud mínima de la contraseña inicial de un usuario creado por administración.
///
/// Es una política de producto, no un valor criptográfico: Argon2id solo impone
/// el máximo. Se mantiene alineada con la del asistente de primera instalación.
pub const MIN_USER_PASSWORD_LEN: usize = 12;

/// Alta de un usuario de la organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewUserRequest {
    /// Nombre visible.
    pub display_name: String,
    /// Correo; se normaliza en la aplicación.
    pub email: String,
    /// Contraseña inicial en claro solo para esta llamada.
    pub password: String,
    /// Instante autoritativo de creación de la credencial.
    pub now: UtcSeconds,
}

/// Cambios permitidos sobre un usuario existente.
///
/// El email es editable porque identifica al usuario para autenticarse; el UUID,
/// la organización y el historial de credenciales no forman parte del contrato.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserUpdate {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nombre visible nuevo.
    pub display_name: Option<String>,
    /// Email nuevo, normalizado por la aplicación.
    pub email: Option<String>,
    /// Estado administrativo nuevo: `ACTIVE` o `INACTIVE`.
    pub status: Option<String>,
}

/// Cambios permitidos sobre un rol existente.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleUpdate {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nombre visible nuevo.
    pub name: Option<String>,
    /// Estado administrativo nuevo: `ACTIVE` o `INACTIVE`.
    pub status: Option<String>,
}

/// Cambios permitidos sobre una membresía existente.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipUpdate {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Estado administrativo nuevo: `ACTIVE` o `INACTIVE`.
    pub status: Option<String>,
    /// Roles de sucursal completos; si falta, se conservan.
    pub role_ids: Option<Vec<String>>,
}

/// Usuario con los roles de organización asignados.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserSummary {
    /// Agregado persistido.
    pub user: PosUser,
    /// Roles de organización vigentes.
    pub roles: Vec<Role>,
}

/// Rol con su conjunto de permisos persistido.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleDetail {
    /// Agregado persistido.
    pub role: Role,
    /// Códigos de permiso del rol, ordenados.
    pub permission_codes: BTreeSet<String>,
}

/// Membresía con los roles de sucursal asignados.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipSummary {
    /// Agregado persistido.
    pub membership: BranchMembership,
    /// Roles de sucursal asignados.
    pub roles: Vec<Role>,
}
