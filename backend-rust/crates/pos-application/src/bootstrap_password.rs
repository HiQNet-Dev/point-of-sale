//! Prueba de posesión de la contraseña inicial, preparada antes de tomar la UoW.
use pos_core::{
    identity::{CredentialAlgorithm, CredentialKind, CredentialState, LocalCredential},
    organization_branch::ApplicationError,
};

/// Puerto criptográfico requerido para aceptar el material inicial.
///
/// La implementación debe comprobar algoritmo, política y contraseña real;
/// validar solo la forma del hash no demuestra que pueda usarse para login.
pub trait BootstrapPasswordVerifier {
    /// Verifica sin registrar ni conservar la contraseña presentada.
    fn verify_initial_password(
        &self,
        credential: &LocalCredential,
        password: &str,
    ) -> Result<(), ApplicationError>;
}

/// Credencial inicial comprobada mediante el puerto criptográfico.
///
/// No tiene constructor de campos públicos ni serde; el secreto en claro nunca
/// forma parte del request persistible. El Debug de LocalCredential redacta hash.
#[derive(Clone, Debug)]
pub struct VerifiedInitialCredential(LocalCredential);

impl VerifiedInitialCredential {
    /// Verifica material nuevo y activo fuera de la transacción de escritura.
    ///
    /// # Errors
    /// Falla cerrado ante PIN, material legado, bloqueo, intentos previos,
    /// revisión distinta de uno o una verificación criptográfica fallida.
    pub fn verify<V: BootstrapPasswordVerifier>(
        verifier: &V,
        credential: LocalCredential,
        password: &str,
    ) -> Result<Self, ApplicationError> {
        if credential.kind() != CredentialKind::Password
            || credential.algorithm() != CredentialAlgorithm::Argon2id
            || credential.state() != CredentialState::Active
            || credential.credential_version() != 1
            || credential.failed_login_attempts() != 0
            || credential.locked_until().is_some()
        {
            return Err(ApplicationError::Storage);
        }
        verifier.verify_initial_password(&credential, password)?;
        Ok(Self(credential))
    }

    /// Material para persistencia interna; nunca un DTO de transporte.
    pub fn credential(&self) -> &LocalCredential {
        &self.0
    }
}
