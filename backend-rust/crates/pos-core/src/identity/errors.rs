use std::fmt;

/// Error de identidad sin hashes, handles ni otros materiales sensibles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// Email ausente, mal formado o fuera del límite heredado.
    InvalidEmail,
    /// Un nombre visible no satisface el value object común.
    InvalidDisplayName,
    /// Hash codificado inválido; nunca se devuelve su contenido.
    InvalidSecretHash,
    /// Identificador de política criptográfica inválido.
    InvalidCredentialPolicy,
    /// Parámetros criptográficos fuera del contrato estructurado.
    InvalidCredentialParameters,
    /// Operación incompatible con el estado actual de la credencial.
    InvalidCredentialState,
    /// Política de bloqueo fuera de rango.
    InvalidLockoutPolicy,
    /// Handle hash de sesión inválido; nunca se devuelve su contenido.
    InvalidSessionHash,
    /// Expiración de sesión no es posterior a su creación.
    InvalidSessionLifetime,
    /// Versión solicitada no coincide con el agregado actual.
    ConcurrentModification,
    /// La versión no puede incrementarse sin overflow.
    RevisionOverflow,
    /// La sesión no está activa en el instante solicitado.
    SessionNotActive,
    /// Usuario, owner o sesión no coincide con el scope solicitado.
    OwnershipMismatch,
    /// El usuario no está activo.
    UserInactive,
    /// El almacenamiento no pudo responder; nunca se interpreta como éxito.
    StorageUnavailable,
    /// El scope solicitado no existe, está inactivo o no pertenece al principal.
    ScopeDenied,
    /// Un permiso no es un código técnico estable.
    InvalidPermission,
    /// No hay capacidad de verificación en el tiempo máximo de espera.
    ///
    /// Es una respuesta segura: no revela si el usuario existe ni el estado de
    /// su credencial, y nunca se interpreta como autenticación correcta.
    VerificationSaturated,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEmail => "email inválido",
            Self::InvalidDisplayName => "nombre visible inválido",
            Self::InvalidSecretHash => "hash de credencial inválido",
            Self::InvalidCredentialPolicy => "política de credencial inválida",
            Self::InvalidCredentialParameters => "parámetros de credencial inválidos",
            Self::InvalidCredentialState => "estado de credencial incompatible",
            Self::InvalidLockoutPolicy => "política de bloqueo inválida",
            Self::InvalidSessionHash => "hash de sesión inválido",
            Self::InvalidSessionLifetime => "vigencia de sesión inválida",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "versión agotada",
            Self::SessionNotActive => "sesión no activa",
            Self::OwnershipMismatch => "identidad fuera de su organización",
            Self::UserInactive => "usuario inactivo",
            Self::StorageUnavailable => "almacenamiento no disponible",
            Self::ScopeDenied => "scope denegado",
            Self::InvalidPermission => "permiso inválido",
            Self::VerificationSaturated => "capacidad de verificación saturada",
        })
    }
}
impl std::error::Error for IdentityError {}
