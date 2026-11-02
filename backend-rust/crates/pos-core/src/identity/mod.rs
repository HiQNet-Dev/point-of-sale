//! Dominio de identidad Local: usuarios, credenciales, sesiones y principal.
//!
//! Reproduce las reglas de Java sin JPA ni SQLx. Los secretos nunca se exponen
//! en las APIs públicas: los hashes se redactan en `Debug` y los handles de
//! sesión jamás salen del adaptador criptográfico.
//!
//! # Organización
//!
//! Por responsabilidad: [`errors`], [`user`], [`credential`], [`session`],
//! [`principal`], [`service`], [`provisioning`], [`roles`] y [`membership`]. La
//! API pública se reexporta aquí.

use serde_json::Value;

mod credential;
mod errors;
mod membership;
mod principal;
mod provisioning;
mod roles;
mod service;
mod session;
mod user;

/// Tamaño máximo serializado de los parámetros de credencial.
pub const MAX_CREDENTIAL_PARAMETERS_BYTES: usize = 2048;
/// Profundidad máxima del árbol de parámetros de credencial.
pub const MAX_CREDENTIAL_PARAMETERS_DEPTH: usize = 3;
/// Longitud máxima del hash de un secreto.
pub const MAX_SECRET_HASH_BYTES: usize = 255;

pub use credential::{
    CredentialAlgorithm, CredentialKind, CredentialParameters, CredentialState, LocalCredential,
    LockoutPolicy, NewCredential, SecretHash,
};
pub use errors::IdentityError;
pub use membership::{BranchMembership, MembershipPatch, membership_changed_fields};
pub use principal::{
    AuthorizationError, PERMISSION_CATALOG, PermissionCode, PrincipalScope, ScopeMode,
    VerifiedPrincipal,
};
pub use provisioning::{
    MAX_PROVISIONING_TTL_SECONDS, MIN_PROVISIONING_TTL_SECONDS, ProvisioningAuthorization,
    ProvisioningError, ProvisioningPurpose, ProvisioningState, TOKEN_DIGEST_HEX_LEN,
};
pub use roles::{Role, RolePatch, role_changed_fields};
pub use service::{LocalIdentityRepository, LocalPrincipalService, PermissionAuthority};
pub use session::{LocalSession, LocalSessionState, SessionHash, UtcSeconds};
pub use user::{NormalizedEmail, PosUser, PosUserPatch, PosUserStatus};

pub(crate) const FORBIDDEN_PARAMETER_TOKENS: [&str; 8] = [
    "password",
    "secret",
    "hash",
    "token",
    "authorization",
    "jwt",
    "salt",
    "credential",
];

pub(crate) fn validate_parameters(value: &Value, depth: usize) -> Result<(), IdentityError> {
    if depth > MAX_CREDENTIAL_PARAMETERS_DEPTH {
        return Err(IdentityError::InvalidCredentialParameters);
    }
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                if is_sensitive_key(key) {
                    return Err(IdentityError::InvalidCredentialParameters);
                }
                validate_parameters(nested, depth + 1)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_parameters(item, depth + 1)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

pub(crate) fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    FORBIDDEN_PARAMETER_TOKENS
        .iter()
        .any(|token| normalized.contains(token))
}

pub(crate) fn validate_policy_id(policy_id: &str) -> Result<(), IdentityError> {
    if policy_id.is_empty()
        || policy_id.len() > 64
        || !policy_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(IdentityError::InvalidCredentialPolicy);
    }
    Ok(())
}
