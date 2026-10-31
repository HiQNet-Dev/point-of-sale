//! Aprovisionamiento inicial canónico sobre una única transacción del llamador.
//!
//! No elige una política comercial: exige la capacidad de `authorize_bootstrap`
//! ya emitida mediante el secreto de despliegue. La credencial inicial exige
//! prueba criptográfica previa y se confirma junto al provisioning; no emite
//! sesiones implícitas. Su activación sigue pendiente de A y D-AUTH-09.

use std::future::Future;

use pos_core::{
    audit::{AuditEventId, AuditEventRecord, AuditOccurredAt, BootstrapProvisioning},
    identity::NormalizedEmail,
    organization_branch::{
        ApplicationError, BusinessCode, DisplayName, EntityId, NewOrganization, Organization,
    },
};

use crate::audit::AuditAppender;
pub use crate::bootstrap_password::{BootstrapPasswordVerifier, VerifiedInitialCredential};

/// Entrada interna de provisioning; sin contraseña en claro ni actor de cliente.
/// No es serializable ni se expone como DTO: incluye material de hash redactado.
#[derive(Clone, Debug)]
pub struct BootstrapRequest {
    /// Instalación que se provisiona.
    pub installation_id: EntityId,
    /// Identidad del evento SYSTEM.
    pub audit_event_id: AuditEventId,
    /// Organización inicial.
    pub organization_id: EntityId,
    /// Datos de dominio de la organización.
    pub organization: NewOrganization,
    /// Administrador inicial.
    pub administrator_id: EntityId,
    /// Nombre visible del administrador.
    pub administrator_name: DisplayName,
    /// Email canónico.
    pub administrator_email: NormalizedEmail,
    /// Rol del administrador.
    pub administrator_role_id: EntityId,
    /// Código estable del rol.
    pub administrator_role_code: BusinessCode,
    /// Huella pública; nunca material de clave privada.
    pub installation_fingerprint: String,
    /// Contraseña inicial ya comprobada; no puede omitirse.
    pub initial_credential: VerifiedInitialCredential,
}

/// Persistencia del provisioning sobre la misma UoW que la auditoría.
pub trait BootstrapTransaction: AuditAppender {
    /// Observa identidad ya provisionada o parcial; no depende de un ID del cliente.
    fn has_initial_identity(
        &mut self,
    ) -> impl Future<Output = Result<bool, ApplicationError>> + Send;

    /// Asigna al administrador el catálogo de permisos vigente, como en Java.
    fn grant_administrator_catalog(
        &mut self,
        request: &BootstrapRequest,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Inserta la credencial comprobada en la misma conexión, sin commit.
    fn insert_initial_credential(
        &mut self,
        request: &BootstrapRequest,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Completa el registro solo después de persistir credencial y grants.
    fn complete_initial_bootstrap(
        &mut self,
        request: &BootstrapRequest,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Persiste el agregado y sus vínculos iniciales, sin completar ni confirmar.
    fn insert_initial_identity(
        &mut self,
        organization: &Organization,
        request: &BootstrapRequest,
        occurred_at: &str,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Consume la autorización de provisioning dentro de la misma UoW.
    ///
    /// Debe validar digest, instalación, finalidad y ventana, aplicar un CAS
    /// sobre el estado pendiente y registrar el evento `CONSUMED`, sin confirmar.
    fn consume_provisioning_authorization(
        &mut self,
        request: &BootstrapRequest,
        authorization_id: &EntityId,
        presented_digest: &str,
        now_epoch: i64,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;

    /// Vincula la preautorización con la instalación canónica ya insertada.
    fn claim_provisional_installation(
        &mut self,
        request: &BootstrapRequest,
        now_epoch: i64,
    ) -> impl Future<Output = Result<(), ApplicationError>> + Send;
}

/// Prueba de autorización presentada por el emisor nativo.
///
/// No se serializa ni viaja por React: el proceso administrativo entrega el
/// token por un canal privado y aquí solo llega su digest.
pub struct ProvisioningProof<'a> {
    /// Autorización emitida previamente.
    pub authorization_id: &'a EntityId,
    /// Digest SHA-256 del token presentado.
    pub presented_digest: &'a str,
    /// Instante actual en segundos UTC, para la ventana y el CAS.
    pub now_epoch: i64,
}

/// Detecta si el provisioning inicial aún es posible dentro de la UoW actual.
///
/// # Errors
/// Propaga indisponibilidad; no trata un fallo de lectura como instalación vacía.
pub async fn requires_provisioning<T: BootstrapTransaction>(
    transaction: &mut T,
) -> Result<bool, ApplicationError> {
    Ok(!transaction.has_initial_identity().await?)
}

/// Crea organización/administrador y registra `bootstrap.completed` como SYSTEM.
///
/// La comprobación y la escritura deben ejecutarse bajo la UoW exclusiva del
/// llamador. No se permite convertir una instalación existente en «nueva»
/// cambiando los identificadores. Ante cualquier fallo el llamador debe revertir.
///
/// # Errors
/// Devuelve Storage para identidad existente/parcial o errores de persistencia;
/// Audit si el evento no se puede construir/escribir. La credencial obligatoria
/// debe pertenecer al administrador de este request. No emite sesiones.
pub async fn provision_initial_installation<T: BootstrapTransaction>(
    transaction: &mut T,
    capability: &BootstrapProvisioning,
    request: &BootstrapRequest,
    occurred_at: &str,
) -> Result<Organization, ApplicationError> {
    let occurred_at = AuditOccurredAt::new(occurred_at).map_err(|_| ApplicationError::Audit)?;
    if !requires_provisioning(transaction).await? {
        return Err(ApplicationError::Storage);
    }
    if request.initial_credential.credential().user_id() != &request.administrator_id {
        return Err(ApplicationError::Storage);
    }
    let organization = Organization::create(
        request.organization_id.clone(),
        request.organization.clone(),
    );
    let event = AuditEventRecord::bootstrap_completed(
        capability,
        request.audit_event_id.clone(),
        organization.id().clone(),
        request.installation_id.clone(),
        occurred_at.clone(),
    )
    .map_err(|_| ApplicationError::Audit)?;
    transaction
        .insert_initial_identity(&organization, request, occurred_at.as_str())
        .await?;
    transaction
        .insert_initial_credential(request, occurred_at.as_str())
        .await?;
    transaction.grant_administrator_catalog(request).await?;
    transaction.append_audit_event(&event).await?;
    transaction
        .complete_initial_bootstrap(request, occurred_at.as_str())
        .await?;
    Ok(organization)
}

/// Crea la instalación inicial exigiendo una autorización de provisioning.
///
/// A diferencia de [`provision_initial_installation`], esta variante no acepta el
/// secreto de despliegue como autoridad: la presentación se valida contra la
/// autorización persistida y se consume en la **misma** transacción que crea la
/// organización, la identidad canónica, el administrador, la credencial, los
/// grants, la auditoría y el bootstrap. Si cualquier paso falla, el llamador
/// revierte y la autorización no queda consumida.
///
/// # Errors
/// Devuelve Storage si la instalación ya existe, la credencial no corresponde al
/// administrador, la autorización no acepta la presentación o el vínculo
/// provisional no puede reclamarse; Audit si el evento no se puede escribir.
pub async fn provision_initial_installation_with_proof<T: BootstrapTransaction>(
    transaction: &mut T,
    capability: &BootstrapProvisioning,
    request: &BootstrapRequest,
    proof: &ProvisioningProof<'_>,
    occurred_at: &str,
) -> Result<Organization, ApplicationError> {
    let occurred_at = AuditOccurredAt::new(occurred_at).map_err(|_| ApplicationError::Audit)?;
    if !requires_provisioning(transaction).await? {
        return Err(ApplicationError::Storage);
    }
    if request.initial_credential.credential().user_id() != &request.administrator_id {
        return Err(ApplicationError::Storage);
    }
    transaction
        .consume_provisioning_authorization(
            request,
            proof.authorization_id,
            proof.presented_digest,
            proof.now_epoch,
        )
        .await?;
    let organization = Organization::create(
        request.organization_id.clone(),
        request.organization.clone(),
    );
    let event = AuditEventRecord::bootstrap_completed(
        capability,
        request.audit_event_id.clone(),
        organization.id().clone(),
        request.installation_id.clone(),
        occurred_at.clone(),
    )
    .map_err(|_| ApplicationError::Audit)?;
    transaction
        .insert_initial_identity(&organization, request, occurred_at.as_str())
        .await?;
    transaction
        .claim_provisional_installation(request, proof.now_epoch)
        .await?;
    transaction
        .insert_initial_credential(request, occurred_at.as_str())
        .await?;
    transaction.grant_administrator_catalog(request).await?;
    transaction.append_audit_event(&event).await?;
    transaction
        .complete_initial_bootstrap(request, occurred_at.as_str())
        .await?;
    Ok(organization)
}
