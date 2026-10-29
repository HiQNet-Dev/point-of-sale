//! Dominio de identidad Local y frontera de principal confiable.
//!
//! No contiene verificadores criptográficos, secretos, sesiones HTTP ni SQLx.
//! Un adaptador de Auth verifica una credencial opaca antes de consultar este
//! dominio; la autorización se resuelve desde datos vigentes, no desde el cliente.

use std::{collections::BTreeSet, fmt};

use crate::{
    audit::{AuditActor, AuditAttributionError, AuditScope},
    organization_branch::{DisplayName, EntityId, java_trim},
};

/// Error de identidad sin hashes, handles ni otros materiales sensibles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// Email ausente, mal formado o fuera del límite heredado.
    InvalidEmail,
    /// Un nombre visible no satisface el value object común.
    InvalidDisplayName,
    /// Hash/referencia de credencial inválido; nunca se devuelve su contenido.
    InvalidCredentialReference,
    /// Identificador de política criptográfica inválido.
    InvalidCredentialPolicy,
    /// Handle hash de sesión inválido; nunca se devuelve su contenido.
    InvalidSessionHash,
    /// Expiración de sesión no es posterior a su creación.
    InvalidSessionLifetime,
    /// Revisión solicitada no coincide con el agregado actual.
    ConcurrentModification,
    /// La revisión no puede incrementarse sin overflow.
    RevisionOverflow,
    /// La sesión no está activa en el instante solicitado.
    SessionNotActive,
    /// Usuario, owner o sesión no coincide con el scope solicitado.
    OwnershipMismatch,
    /// El usuario no está activo.
    UserInactive,
    /// Un permiso no es un código técnico estable.
    InvalidPermission,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEmail => "email inválido",
            Self::InvalidDisplayName => "nombre visible inválido",
            Self::InvalidCredentialReference => "referencia de credencial inválida",
            Self::InvalidCredentialPolicy => "política de credencial inválida",
            Self::InvalidSessionHash => "hash de sesión inválido",
            Self::InvalidSessionLifetime => "vigencia de sesión inválida",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "revisión agotada",
            Self::SessionNotActive => "sesión no activa",
            Self::OwnershipMismatch => "identidad fuera de su organización",
            Self::UserInactive => "usuario inactivo",
            Self::InvalidPermission => "permiso inválido",
        })
    }
}
impl std::error::Error for IdentityError {}

/// Email normalizado como Java: `String.trim()` y minúsculas con semántica Root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedEmail(String);

impl NormalizedEmail {
    /// Valida la forma mínima heredada (`local@domain.tld`) y 254 unidades UTF-16.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        let normalized = java_trim(value).to_lowercase();
        let mut parts = normalized.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        if parts.next().is_some()
            || local.is_empty()
            || domain.is_empty()
            || !domain.contains('.')
            || normalized.chars().any(char::is_whitespace)
            || normalized.contains('\0')
            || normalized.encode_utf16().count() > 254
        {
            return Err(IdentityError::InvalidEmail);
        }
        Ok(Self(normalized))
    }

    /// Representación segura para búsquedas y unicidad por Organization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Estado administrativo del usuario Local.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PosUserStatus {
    /// Usuario disponible para autenticación y autorización.
    Active,
    /// Usuario conservado históricamente, sin capacidad operativa.
    Inactive,
}

/// Patch permitido: ni UUID ni Organization pueden modificarse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosUserPatch {
    /// Revisión previamente leída.
    pub expected_revision: u64,
    /// Nombre visible opcional.
    pub display_name: Option<DisplayName>,
    /// Email opcional ya validado.
    pub email: Option<NormalizedEmail>,
    /// Estado administrativo opcional.
    pub status: Option<PosUserStatus>,
}

/// Usuario Local, propietario de una sola Organization e independiente de credenciales.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosUser {
    id: EntityId,
    organization_id: EntityId,
    display_name: DisplayName,
    email: NormalizedEmail,
    status: PosUserStatus,
    revision: u64,
}

impl PosUser {
    /// Crea una identidad activa; credenciales se asocian posteriormente por UUID.
    pub fn create(
        id: EntityId,
        organization_id: EntityId,
        display_name: DisplayName,
        email: NormalizedEmail,
    ) -> Self {
        Self {
            id,
            organization_id,
            display_name,
            email,
            status: PosUserStatus::Active,
            revision: 0,
        }
    }
    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Owner inmutable.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Nombre visible no secreto.
    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }
    /// Email normalizado.
    pub fn email(&self) -> &NormalizedEmail {
        &self.email
    }
    /// Estado actual.
    pub fn status(&self) -> PosUserStatus {
        self.status
    }
    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Indica si la identidad puede autenticarse/autorizase.
    pub fn is_active(&self) -> bool {
        self.status == PosUserStatus::Active
    }

    /// Aplica un patch atómico sin permitir mover al usuario ni modificar su UUID.
    pub fn patch(&mut self, patch: PosUserPatch) -> Result<bool, IdentityError> {
        if patch.expected_revision != self.revision {
            return Err(IdentityError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.display_name {
            changed |= next.display_name != value;
            next.display_name = value;
        }
        if let Some(value) = patch.email {
            changed |= next.email != value;
            next.email = value;
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(IdentityError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

/// Algoritmos de credencial aceptables para transición, no una política de hashing activa.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    /// Formato BCrypt histórico que solo puede verificarse y migrarse tras aprobación.
    LegacyBcrypt,
    /// Formato Argon2id candidato para credenciales nuevas.
    Argon2id,
}

/// Lifecycle de una credencial; `ResetRequired` nunca prueba fallbacks inseguros.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialState {
    /// Credencial apta para verificación por un adaptador autorizado.
    Ready,
    /// Hash/política no confiable; exige reset sin fallback.
    ResetRequired,
    /// Credencial deshabilitada por un flujo administrativo autorizado.
    Disabled,
}

/// Referencia opaca al hash codificado y parámetros; su `Debug` nunca revela contenido.
#[derive(Clone, Eq, PartialEq)]
pub struct CredentialReference(String);
impl CredentialReference {
    /// Acepta una referencia/PHC ya producida por un verificador autorizado, hasta 255 bytes.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty() || value.len() > 255 || value.contains(['\0', '\n', '\r']) {
            return Err(IdentityError::InvalidCredentialReference);
        }
        Ok(Self(value.to_owned()))
    }
    /// Entrega el material solo al adaptador criptográfico dentro de una frontera de confianza.
    #[allow(dead_code)]
    pub(crate) fn exposed_to_verifier(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for CredentialReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialReference(REDACTED)")
    }
}

/// Credencial Local separada de la identidad administrativa.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCredential {
    id: EntityId,
    user_id: EntityId,
    kind: CredentialKind,
    reference: CredentialReference,
    policy_id: String,
    state: CredentialState,
    revision: u64,
}
impl LocalCredential {
    /// Asocia una única credencial a un usuario ya owner de una Organization.
    pub fn create(
        id: EntityId,
        user_id: EntityId,
        kind: CredentialKind,
        reference: CredentialReference,
        policy_id: &str,
    ) -> Result<Self, IdentityError> {
        if policy_id.is_empty()
            || policy_id.len() > 64
            || !policy_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(IdentityError::InvalidCredentialPolicy);
        }
        Ok(Self {
            id,
            user_id,
            kind,
            reference,
            policy_id: policy_id.to_owned(),
            state: CredentialState::Ready,
            revision: 0,
        })
    }
    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Usuario owner inmutable.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
    /// Algoritmo declarado; la verificación real se mantiene en el adaptador aprobado.
    pub fn kind(&self) -> CredentialKind {
        self.kind
    }
    /// Estado de ciclo de vida.
    pub fn state(&self) -> CredentialState {
        self.state
    }
    /// Política declarada, no secreta.
    pub fn policy_id(&self) -> &str {
        &self.policy_id
    }
    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Marca la credencial como reset requerido de manera idempotente.
    pub fn require_reset(&mut self, expected_revision: u64) -> Result<bool, IdentityError> {
        if expected_revision != self.revision {
            return Err(IdentityError::ConcurrentModification);
        }
        if self.state == CredentialState::ResetRequired {
            return Ok(false);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(IdentityError::RevisionOverflow)?;
        self.state = CredentialState::ResetRequired;
        self.revision = revision;
        Ok(true)
    }
}

/// Instant UTC lógico para decisiones de expiración, expresado como segundos Unix.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UtcSeconds(i64);
impl UtcSeconds {
    /// Construye un instante UTC no negativo.
    pub fn new(value: i64) -> Result<Self, IdentityError> {
        if value < 0 {
            return Err(IdentityError::InvalidSessionLifetime);
        }
        Ok(Self(value))
    }

    /// Segundos Unix.
    pub fn get(self) -> i64 {
        self.0
    }
}

/// Hash SHA-256 hexadecimal de un handle opaco; el handle crudo jamás entra al dominio.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionHandleHash(String);
impl SessionHandleHash {
    /// Valida exactamente 32 bytes SHA-256 codificados en hexadecimal minúsculo.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(IdentityError::InvalidSessionHash);
        }
        Ok(Self(value.to_owned()))
    }
    /// Uso exclusivo por repositorios/adaptadores de lookup; nunca para respuestas o logs.
    #[allow(dead_code)]
    pub(crate) fn for_lookup(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SessionHandleHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionHandleHash(REDACTED)")
    }
}

/// Estado efectivo de una sesión Local en un instante.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalSessionState {
    /// Sesión vigente y no revocada.
    Active,
    /// Sesión revocada de forma explícita.
    Revoked,
    /// Sesión que agotó su vigencia temporal.
    Expired,
}

/// Sesión Local de handle opaco, ligada a usuario y Organization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSession {
    id: EntityId,
    user_id: EntityId,
    organization_id: EntityId,
    handle_hash: SessionHandleHash,
    created_at: UtcSeconds,
    expires_at: UtcSeconds,
    revoked_at: Option<UtcSeconds>,
    revision: u64,
}
impl LocalSession {
    /// Crea una sesión todavía activa; se exige una expiración estrictamente futura.
    pub fn create(
        id: EntityId,
        user_id: EntityId,
        organization_id: EntityId,
        handle_hash: SessionHandleHash,
        created_at: UtcSeconds,
        expires_at: UtcSeconds,
    ) -> Result<Self, IdentityError> {
        if expires_at <= created_at {
            return Err(IdentityError::InvalidSessionLifetime);
        }
        Ok(Self {
            id,
            user_id,
            organization_id,
            handle_hash,
            created_at,
            expires_at,
            revoked_at: None,
            revision: 0,
        })
    }
    /// Identidad estable que `audit_event.session_id` podrá referenciar.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Usuario owner inmutable.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
    /// Organization derivada y persistida para validar el scope sin confiar en cliente.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Inicio UTC lógico.
    pub fn created_at(&self) -> UtcSeconds {
        self.created_at
    }
    /// Expiración UTC lógica.
    pub fn expires_at(&self) -> UtcSeconds {
        self.expires_at
    }
    /// Momento de revocación, si existe.
    pub fn revoked_at(&self) -> Option<UtcSeconds> {
        self.revoked_at
    }
    /// Revisión de sesión.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Estado derivado: revocación gana a expiración.
    pub fn state_at(&self, now: UtcSeconds) -> LocalSessionState {
        if self.revoked_at.is_some() {
            LocalSessionState::Revoked
        } else if now >= self.expires_at {
            LocalSessionState::Expired
        } else {
            LocalSessionState::Active
        }
    }
    /// Rechaza sesión expirada/revocada o de otro user/org.
    pub fn validate_for(
        &self,
        user_id: &EntityId,
        organization_id: &EntityId,
        now: UtcSeconds,
    ) -> Result<(), IdentityError> {
        if &self.user_id != user_id || &self.organization_id != organization_id {
            return Err(IdentityError::OwnershipMismatch);
        }
        if self.state_at(now) != LocalSessionState::Active {
            return Err(IdentityError::SessionNotActive);
        }
        Ok(())
    }
    /// Revoca con CAS, sin revivir nunca una sesión.
    pub fn revoke(
        &mut self,
        expected_revision: u64,
        now: UtcSeconds,
    ) -> Result<bool, IdentityError> {
        if expected_revision != self.revision {
            return Err(IdentityError::ConcurrentModification);
        }
        if self.revoked_at.is_some() {
            return Ok(false);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(IdentityError::RevisionOverflow)?;
        self.revoked_at = Some(now);
        self.revision = revision;
        Ok(true)
    }
    /// Sólo para el repositorio de sesión; no se expone en DTOs/logs.
    #[allow(dead_code)]
    pub(crate) fn handle_hash(&self) -> &SessionHandleHash {
        &self.handle_hash
    }
}

/// Código de permiso resuelto desde el catálogo vigente de base de datos.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PermissionCode(String);
impl PermissionCode {
    /// Acepta códigos técnicos ASCII separados por puntos, sin wildcard.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || value.len() > 64
            || value.split('.').any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
        {
            return Err(IdentityError::InvalidPermission);
        }
        Ok(Self(value.to_owned()))
    }
    /// Código para comparar una decisión de autorización.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Principal ya autenticado y scoped. Sus campos no son elegibles desde transporte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPrincipal {
    user_id: EntityId,
    organization_id: EntityId,
    session_id: EntityId,
    permissions: BTreeSet<PermissionCode>,
}
impl VerifiedPrincipal {
    /// Usuario atribuido por sesión validada.
    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }
    /// Organización validada contra la sesión y usuario.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Sesión Local que podrá correlacionar hechos auditables.
    pub fn session_id(&self) -> &EntityId {
        &self.session_id
    }
    /// Decide un permiso solamente entre las grants consultadas de DB.
    pub fn allows(&self, permission: &PermissionCode) -> bool {
        self.permissions.contains(permission)
    }
    /// Verifica que un scope de operación pertenece a la organización del principal.
    ///
    /// Un principal autenticado nunca habilita otra Organization; la sucursal
    /// concreta se resuelve además contra `PermissionAuthority`.
    pub fn permits_scope(&self, scope: &AuditScope) -> bool {
        self.organization_id == scope.organization_id
    }
    /// Construye USER auditado sin recibir actor desde el cliente.
    pub fn audit_actor(&self) -> AuditActor {
        AuditActor::user(self.user_id.clone())
    }
}

/// Lecturas DB-authoritative requeridas para construir un principal.
pub trait LocalIdentityRepository {
    /// Busca por hash de handle ya calculado por el adaptador criptográfico.
    fn find_session_by_hash(
        &self,
        hash: &SessionHandleHash,
    ) -> Result<Option<LocalSession>, IdentityError>;
    /// Busca el usuario dentro de la Organization derivada de la sesión.
    fn find_user(
        &self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Option<PosUser>, IdentityError>;
}
/// Resolución de permisos vigente en DB; no admite grants desde DTO/JWT.
pub trait PermissionAuthority {
    /// Devuelve las grants del usuario en Organization y scope opcional de Branch.
    fn resolve_permissions(
        &self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: Option<&EntityId>,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError>;
}

/// Coordinación de autenticación local: sesión validada → usuario activo → permisos vigentes.
pub struct LocalPrincipalService<R, P> {
    repository: R,
    permissions: P,
}
impl<R: LocalIdentityRepository, P: PermissionAuthority> LocalPrincipalService<R, P> {
    /// Ensambla adaptadores explícitos; no existe implementación allow-all.
    pub fn new(repository: R, permissions: P) -> Self {
        Self {
            repository,
            permissions,
        }
    }
    /// Construye un principal solamente si la sesión, owner, usuario y permisos son válidos.
    pub fn authenticate(
        &self,
        hash: &SessionHandleHash,
        now: UtcSeconds,
        branch_id: Option<&EntityId>,
    ) -> Result<VerifiedPrincipal, AuditAttributionError> {
        let session = self
            .repository
            .find_session_by_hash(hash)
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?
            .ok_or(AuditAttributionError::Unauthenticated)?;
        let user = self
            .repository
            .find_user(session.organization_id(), session.user_id())
            .map_err(|_| AuditAttributionError::UnattributablePrincipal)?
            .ok_or(AuditAttributionError::Unauthenticated)?;
        session
            .validate_for(user.id(), user.organization_id(), now)
            .map_err(|error| match error {
                IdentityError::SessionNotActive => AuditAttributionError::Unauthenticated,
                _ => AuditAttributionError::UnattributablePrincipal,
            })?;
        if !user.is_active() {
            return Err(AuditAttributionError::Unauthenticated);
        }
        let permissions = self
            .permissions
            .resolve_permissions(user.id(), user.organization_id(), branch_id)
            .map_err(|_| AuditAttributionError::ScopeDenied)?;
        Ok(VerifiedPrincipal {
            user_id: user.id().clone(),
            organization_id: user.organization_id().clone(),
            session_id: session.id().clone(),
            permissions,
        })
    }
}
