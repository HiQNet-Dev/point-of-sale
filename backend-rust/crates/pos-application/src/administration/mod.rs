//! Administración de identidad: usuarios, roles y membresías de sucursal.
//!
//! # Por qué una composición asíncrona propia
//!
//! Igual que la configuración del negocio, esta composición reutiliza los
//! agregados de dominio (`PosUser`, `Role`, `BranchMembership`) sobre los puertos
//! asíncronos que implementa `SqliteUnitOfWork`. No duplica invariantes: los
//! agregados deciden qué cambia y qué revisión resulta, y la aplicación solo
//! coordina autorización, CAS, auditoría y transacción.
//!
//! # Modelo de autorización
//!
//! | Operación | Permiso |
//! | --- | --- |
//! | Listar y consultar usuarios | `user.read` |
//! | Crear usuarios | `user.create` |
//! | Editar nombre, email y estado | `user.update` |
//! | Reemplazar roles de organización | `role.assign` |
//! | Listar y consultar roles | `role.read` |
//! | Crear roles | `role.create` |
//! | Editar nombre y estado de roles | `role.update` |
//! | Reemplazar permisos de un rol | `role.permissions.manage` |
//! | Consultar el catálogo de permisos | `permission.read` |
//! | Listar membresías | `branch.membership.read` |
//! | Crear membresías | `branch.membership.create` |
//! | Editar estado de membresías | `branch.membership.update` |
//! | Reemplazar roles de una membresía | `branch.membership.roles.manage` |
//!
//! # Continuidad de la administración
//!
//! Un administrador no puede modificar sus propios roles ni desactivar su propia
//! cuenta, y ninguna operación puede dejar la organización sin ningún usuario
//! ACTIVO con un rol ACTIVO que conceda `user.update`. Sin esa garantía la
//! instalación Free Local quedaría sin forma de recuperar la administración.
//!
//! # Fuera de alcance
//!
//! No existe restablecimiento administrativo de contraseña: requiere un contrato
//! propio con pruebas de seguridad. Tampoco se implementa todavía la rotación de
//! hashes de credenciales existentes, que depende de la ratificación D-AUTH-09.

mod application;
mod dto;
mod memberships;
mod ports;
mod roles;
mod users;

pub use application::AdministrationApplication;
pub use dto::{
    MIN_USER_PASSWORD_LEN, MembershipSummary, MembershipUpdate, NewUserRequest, RoleDetail,
    RoleUpdate, UserSummary, UserUpdate,
};
pub use ports::{AdministrationTransaction, AdministrationUnitOfWork, PasswordHasher};
