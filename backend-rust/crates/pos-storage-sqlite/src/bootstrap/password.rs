//! Adaptador criptográfico del provisioning candidato; nunca conectado a producto.
use pos_application::bootstrap::{BootstrapPasswordVerifier, VerifiedInitialCredential};
use pos_core::{
    identity::{
        CredentialAlgorithm, CredentialKind, CredentialParameters, LocalCredential, NewCredential,
        UtcSeconds,
    },
    organization_branch::{ApplicationError, EntityId},
};
use pos_credentials::{
    Argon2Policy, VerificationOutcome, VerifyRequest, hash_password, verify_credential,
};
use serde_json::json;

/// Preparador explícito del esquema candidato. Las políticas LAB no se activan
/// como políticas comerciales; este adaptador solo se ensambla en pruebas/harness.
pub struct CandidateBootstrapPasswords {
    policy: Argon2Policy,
}
impl CandidateBootstrapPasswords {
    /// Selección explícita del candidato de pruebas, nunca una política de producto.
    pub fn lab_v1() -> Self {
        Self::new(Argon2Policy::LAB_V1)
    }

    /// Requiere selección explícita, sin defaults comerciales.
    pub fn new(policy: Argon2Policy) -> Self {
        Self { policy }
    }

    /// Genera sal CSPRNG, material nuevo y prueba que la contraseña lo verifica.
    /// Ejecutar fuera de la UoW y del hilo de UI, bajo admisión acotada.
    ///
    /// # Errors
    /// Colapsa errores sin contraseña/hash en mensajes o logs.
    pub fn prepare(
        &self,
        credential_id: EntityId,
        user_id: EntityId,
        password: &str,
        now: UtcSeconds,
    ) -> Result<VerifiedInitialCredential, ApplicationError> {
        let secret_hash =
            hash_password(password, self.policy).map_err(|_| ApplicationError::Storage)?;
        let credential = LocalCredential::create(NewCredential {
            id: credential_id,
            user_id,
            kind: CredentialKind::Password,
            algorithm: CredentialAlgorithm::Argon2id,
            secret_hash,
            parameters: self.parameters()?,
            policy_id: self.policy.id().into(),
            created_at: now,
        })
        .map_err(|_| ApplicationError::Storage)?;
        VerifiedInitialCredential::verify(self, credential, password)
    }

    fn parameters(&self) -> Result<CredentialParameters, ApplicationError> {
        CredentialParameters::new(&json!({"m_cost":self.policy.memory_kib(),"t_cost":self.policy.iterations(),"p_cost":self.policy.lanes()}))
            .map_err(|_| ApplicationError::Storage)
    }
}
impl BootstrapPasswordVerifier for CandidateBootstrapPasswords {
    fn verify_initial_password(
        &self,
        credential: &LocalCredential,
        password: &str,
    ) -> Result<(), ApplicationError> {
        if credential.policy_id() != self.policy.id()
            || credential.parameters() != &self.parameters()?
        {
            return Err(ApplicationError::Storage);
        }
        match verify_credential(&VerifyRequest {
            purpose: CredentialKind::Password,
            algorithm: credential.algorithm(),
            stored: credential.secret_hash(),
            password,
        })
        .map_err(|_| ApplicationError::Storage)?
        {
            VerificationOutcome::Verified {
                rehash_required: false,
            } => Ok(()),
            VerificationOutcome::Verified {
                rehash_required: true,
            }
            | VerificationOutcome::Rejected
            | VerificationOutcome::Unusable => Err(ApplicationError::Storage),
        }
    }
}
