//! Puertos de la administración de identidad.
//!
//! Declarados en la capa que los consume. El adaptador SQLite los implementa
//! sobre la misma frontera transaccional que el resto de módulos: una conexión
//! con `BEGIN IMMEDIATE`, la mutación, la auditoría y un único commit.
//!
//! # Ownership
//!
//! Toda lectura y escritura incluye `organization_id`: una fila de otra
//! organización se comporta como inexistente y las claves foráneas compuestas del
//! esquema lo respaldan.

use std::future::Future;

use pos_core::{
    identity::{BranchMembership, LocalCredential, NormalizedEmail, PosUser, Role, UtcSeconds},
    organization_branch::{ApplicationError, Branch, BusinessCode, EntityId},
};

use crate::{CasOutcome, audit::AuditAppender};

/// Preparación criptográfica de una credencial nueva.
///
/// Se ejecuta fuera de la transacción de escritura porque el coste de Argon2id no
/// debe retener la reserva de SQLite.
pub trait PasswordHasher {
    /// Deriva el material de la contraseña inicial de un usuario nuevo.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::InvalidAdministrationInput`] si la contraseña
    /// no cumple la política y [`ApplicationError::Storage`] si la criptografía
    /// no está disponible. Nunca incluye el secreto en el error.
    fn prepare(
        &self,
        credential_id: EntityId,
        user_id: EntityId,
        password: &str,
        now: UtcSeconds,
    ) -> impl Future<Output = Result<LocalCredential, ApplicationError>> + Send;
}

/// Operaciones transaccionales de administración de identidad.
pub trait AdministrationTransaction: AuditAppender + Send {
    // ---- Usuarios ----------------------------------------------------------
    /// Usuario dentro de la organización.
    fn get_user(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<Option<PosUser>, ApplicationError>> + Send;
    /// Usuarios de la organización ordenados por email.
    fn list_users(
        &mut self,
        organization_id: &EntityId,
        limit: u32,
        offset: u32,
    ) -> impl Future<Output = Result<Vec<PosUser>, ApplicationError>> + Send;
    /// Comprueba si el email ya está ocupado por otro usuario de la organización.
    fn user_email_exists(
        &mut self,
        organization_id: &EntityId,
        email: &NormalizedEmail,
        excluding: Option<&EntityId>,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Inserta un usuario nuevo.
    fn insert_user(
        &mut self,
        user: &PosUser,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Aplica CAS sobre la revisión previa.
    fn update_user_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        user: &PosUser,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Inserta la credencial inicial de un usuario.
    fn insert_credential(
        &mut self,
        credential: &LocalCredential,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Indica si el usuario ya dispone de credencial de contraseña.
    fn user_has_password(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Revoca todas las sesiones vigentes de un usuario.
    fn revoke_user_sessions(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
        occurred_at: &str,
    ) -> impl Future<Output = Result<u64, ApplicationError>> + Send;
    /// Cuenta los administradores efectivos: usuarios ACTIVOS con rol ACTIVO que
    /// concede `user.update`.
    fn count_active_administrators(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<u64, ApplicationError>> + Send;
    /// Indica si un usuario ACTIVO concreto es administrador efectivo.
    fn is_active_administrator(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Cuenta los administradores efectivos que dependen de roles distintos del
    /// indicado, para proteger la continuidad al editar sus permisos.
    fn count_active_administrators_excluding_role(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
    ) -> impl Future<Output = Result<u64, ApplicationError>> + Send;

    // ---- Roles -------------------------------------------------------------
    /// Rol de la organización.
    fn get_role(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Role>, ApplicationError>> + Send;
    /// Roles de la organización ordenados por código.
    fn list_roles(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<Role>, ApplicationError>> + Send;
    /// Comprueba si el código de rol ya existe en la organización.
    fn role_code_exists(
        &mut self,
        organization_id: &EntityId,
        code: &BusinessCode,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Inserta un rol nuevo.
    fn insert_role(
        &mut self,
        role: &Role,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Aplica CAS sobre la revisión previa del rol.
    fn update_role_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        role: &Role,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Códigos de permiso asignados al rol.
    fn role_permission_codes(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<String>, ApplicationError>> + Send;
    /// Reemplaza el conjunto de permisos del rol.
    fn replace_role_permissions(
        &mut self,
        organization_id: &EntityId,
        role_id: &EntityId,
        codes: &[String],
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Indica si el código existe en el catálogo de permisos del sistema.
    fn permission_exists(
        &mut self,
        code: &str,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;
    /// Catálogo de permisos del sistema, ordenado por código.
    fn list_permissions(
        &mut self,
    ) -> impl Future<Output = Result<Vec<(String, String)>, ApplicationError>> + Send;

    // ---- Roles de organización por usuario ----------------------------------
    /// Identidades de los roles de organización asignados al usuario.
    fn organization_role_ids(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<EntityId>, ApplicationError>> + Send;
    /// Reemplaza el conjunto de roles de organización del usuario.
    fn replace_organization_roles(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
        role_ids: &[EntityId],
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    // ---- Membresías de sucursal --------------------------------------------
    /// Sucursal dentro de la organización, para validar el alta de membresías.
    fn get_branch(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Branch>, ApplicationError>> + Send;
    /// Membresías de la organización.
    fn list_memberships(
        &mut self,
        organization_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<BranchMembership>, ApplicationError>> + Send;
    /// Membresía por identidad.
    fn get_membership(
        &mut self,
        organization_id: &EntityId,
        membership_id: &EntityId,
    ) -> impl Future<Output = Result<Option<BranchMembership>, ApplicationError>> + Send;
    /// Membresía de un usuario en una sucursal concreta.
    fn find_membership(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<Option<BranchMembership>, ApplicationError>> + Send;
    /// Inserta una membresía nueva.
    fn insert_membership(
        &mut self,
        membership: &BranchMembership,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Aplica CAS sobre la revisión previa de la membresía.
    fn update_membership_cas(
        &mut self,
        organization_id: &EntityId,
        previous_revision: u64,
        membership: &BranchMembership,
        occurred_at: &str,
    ) -> impl Future<Output = Result<CasOutcome, ApplicationError>> + Send;
    /// Identidades de los roles de sucursal de la membresía.
    fn membership_role_ids(
        &mut self,
        organization_id: &EntityId,
        membership_id: &EntityId,
    ) -> impl Future<Output = Result<Vec<EntityId>, ApplicationError>> + Send;
    /// Reemplaza el conjunto de roles de la membresía.
    fn replace_membership_roles(
        &mut self,
        organization_id: &EntityId,
        membership_id: &EntityId,
        role_ids: &[EntityId],
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}

/// Unidad de trabajo de la administración de identidad.
pub trait AdministrationUnitOfWork {
    /// Transacción que comparte mutación, CAS y auditoría.
    type Transaction: AdministrationTransaction + Send;

    /// Abre la transacción de escritura.
    fn begin_transaction(
        &self,
    ) -> impl Future<Output = Result<Self::Transaction, ApplicationError>> + Send;
    /// Confirma una operación completa.
    fn commit_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
    /// Revierte una operación fallida.
    fn rollback_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}
