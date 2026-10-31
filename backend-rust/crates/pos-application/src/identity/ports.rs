//! Puertos de autenticación Local.
//!
//! La criptografía, el SQL y la resolución DB-authoritative de permisos viven en
//! los adaptadores. Esta capa solo declara qué necesita el caso de uso y nunca
//! recibe identificadores de actor, organización ni permisos desde el cliente.

use std::{collections::BTreeSet, future::Future};

use pos_core::{
    identity::{
        LocalCredential, LocalSession, NormalizedEmail, PermissionCode, PosUser, SessionHash,
        UtcSeconds, VerifiedPrincipal,
    },
    organization_branch::{ApplicationError, Branch, EntityId},
};

use crate::audit::AuditAppender;

/// Usuario y credencial resueltos por email fuera de la transacción de escritura.
///
/// El snapshot permite verificar la contraseña sin retener el bloqueo de
/// escritura de SQLite; la transacción posterior revalida versión y estado.
#[derive(Clone, Debug)]
pub struct LoginSnapshot {
    /// Usuario resuelto.
    pub user: PosUser,
    /// Credencial de contraseña vigente.
    pub credential: LocalCredential,
}

/// Lecturas de identidad previas a la transacción.
pub trait LoginDirectory {
    /// Resuelve exactamente un usuario por email normalizado.
    ///
    /// Devuelve `Ok(None)` cuando no hay coincidencia **o** cuando hay más de
    /// una: el caso de uso aplica la misma respuesta genérica en ambos casos.
    fn find_login_snapshot(
        &self,
        email: &NormalizedEmail,
    ) -> impl Future<Output = Result<Option<LoginSnapshot>, ApplicationError>> + Send;

    /// Usuario dentro de su organización.
    fn user_by_id(
        &self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<Option<PosUser>, ApplicationError>> + Send;

    /// Revalida una sesión y resuelve permisos vigentes.
    fn principal_for_hash(
        &self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
    ) -> impl Future<Output = Result<VerifiedPrincipal, ApplicationError>> + Send;

    /// Sucursal dentro de una organización concreta.
    fn branch_by_id(
        &self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> impl Future<Output = Result<Option<Branch>, ApplicationError>> + Send;
}

/// Resultado criptográfico seguro, sin material ni motivos detallados.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasswordVerdict {
    /// El secreto coincide con el material almacenado.
    Verified,
    /// El secreto no coincide o el material no es verificable.
    Rejected,
}

/// Verificación de contraseña fuera del hilo de la transacción.
pub trait PasswordGate {
    /// Verifica el secreto contra la credencial cargada.
    fn verify(
        &self,
        credential: &LocalCredential,
        password: &str,
    ) -> impl Future<Output = Result<PasswordVerdict, ApplicationError>> + Send;

    /// Consume un costo criptográfico comparable cuando no hay usuario.
    ///
    /// Iguala el tiempo de respuesta del camino «usuario inexistente» para no
    /// ofrecer una señal de enumeración por latencia.
    fn absorb_absent(
        &self,
        password: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}

/// Sesión recién emitida y el handle que solo verá la capa nativa.
#[derive(Clone, Debug)]
pub struct IssuedSession {
    /// Sesión de dominio con digest ya calculado.
    pub session: LocalSession,
    /// Digest que se persiste; el handle crudo nunca toca SQLite.
    pub hash: SessionHash,
    /// Secreto opaco que se entrega al shell nativo; nunca se persiste.
    pub handle: String,
}

/// Generación criptográfica de handles y digests de sesión.
pub trait SessionSecrets {
    /// Emite una sesión activa con entropía del sistema operativo.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Storage`] si no hay entropía disponible o la
    /// ventana temporal no es coherente.
    fn issue(
        &self,
        user_id: EntityId,
        organization_id: EntityId,
        now: UtcSeconds,
        ttl_seconds: i64,
    ) -> Result<IssuedSession, ApplicationError>;
}

/// Operaciones transaccionales de identidad.
pub trait IdentityTransaction: AuditAppender + Send {
    /// Relee la credencial de contraseña del usuario dentro de la UoW.
    fn reload_credential(
        &mut self,
        user: &PosUser,
    ) -> impl Future<Output = Result<Option<LocalCredential>, ApplicationError>> + Send;

    /// Persiste el estado de la credencial con CAS sobre `credential_version`.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::ConcurrentModification`] si otro intento
    /// cambió la fila entre la lectura y la escritura.
    fn save_credential(
        &mut self,
        user: &PosUser,
        credential: &LocalCredential,
        expected_version: u64,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Inserta una sesión activa; el digest es la única columna sensible.
    fn insert_session(
        &mut self,
        session: &LocalSession,
        hash: &SessionHash,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Revalida sesión y resuelve permisos sobre la conexión transaccional.
    fn principal_for_hash(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
    ) -> impl Future<Output = Result<VerifiedPrincipal, ApplicationError>> + Send;

    /// Revoca la sesión del principal y devuelve las filas afectadas.
    fn revoke_session(
        &mut self,
        principal: &VerifiedPrincipal,
        now: UtcSeconds,
    ) -> impl Future<Output = Result<u64, ApplicationError>> + Send;

    /// Revoca por digest sin exigir un principal vigente (logout idempotente).
    fn revoke_session_by_hash(
        &mut self,
        hash: &SessionHash,
        now: UtcSeconds,
    ) -> impl Future<Output = Result<u64, ApplicationError>> + Send;

    /// Revoca todas las sesiones vigentes del usuario del principal.
    fn revoke_all_sessions(
        &mut self,
        principal: &VerifiedPrincipal,
        now: UtcSeconds,
    ) -> impl Future<Output = Result<u64, ApplicationError>> + Send;
}

/// Unidad de trabajo de identidad.
pub trait IdentityUnitOfWork {
    /// Transacción entregada por el adaptador.
    type Transaction: IdentityTransaction + Send;

    /// Abre la transacción de escritura.
    fn begin_transaction(
        &self,
    ) -> impl Future<Output = Result<Self::Transaction, ApplicationError>> + Send;

    /// Confirma una sola vez.
    fn commit_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Revierte antes de cualquier confirmación.
    fn rollback_transaction(
        &self,
        transaction: Self::Transaction,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}

/// Permisos efectivos de una sesión, ya resueltos por SQLite.
pub type EffectivePermissions = BTreeSet<PermissionCode>;
