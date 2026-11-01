//! Configuración del negocio sobre la sesión autenticada.
//!
//! Cada comando revalida el principal en SQLite y delega en
//! `pos-application::organization::administration`. El cliente no aporta
//! organización, actor ni permisos: solo datos de configuración y la revisión que
//! leyó.
//!
//! # Contrato de edición
//!
//! Los DTO de entrada no contienen identidad, código, estado ni moneda. Al
//! rechazar campos desconocidos, un renderer comprometido tampoco puede
//! enviarlos: la restricción no depende de que la interfaz oculte un control.

use pos_application::organization::administration::{
    BranchConfiguration, BranchConfigurationUpdate, BranchCreation,
    OrganizationAdministrationApplication, OrganizationConfigurationUpdate, TimezoneUpdate,
};
use pos_core::organization_branch::{EntityId, Organization};
use pos_storage_sqlite::{SqliteOrganizationAdministration, SqliteUnitOfWork};

use crate::{
    auth::SecureIdentities,
    composition::LocalState,
    dto::{
        BranchAdminDto, CreateBranchRequestDto, IpcError, OrganizationDto,
        TimezoneOverrideRequestDto, UpdateBranchRequestDto, UpdateOrganizationRequestDto,
    },
};

impl LocalState {
    /// Lee la configuración de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Unauthorized`] sin sesión y [`IpcError::Forbidden`]
    /// sin `organization.read`.
    pub async fn organization_get(&self) -> Result<OrganizationDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let organization = self.administration()?.get_organization(&principal).await?;
        Ok(OrganizationDto::from(&organization))
    }

    /// Actualiza la configuración editable de la organización.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `organization.update`,
    /// [`IpcError::ValidationError`] si un valor no cumple el contrato y
    /// [`IpcError::Conflict`] si la revisión enviada está obsoleta.
    pub async fn organization_update(
        &self,
        request: UpdateOrganizationRequestDto,
    ) -> Result<OrganizationDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let updated = self
            .administration()?
            .update_organization(
                &principal,
                &mut identities,
                OrganizationConfigurationUpdate {
                    expected_revision: revision(&request.expected_revision)?,
                    name: request.name,
                    timezone: request.timezone,
                    locale: request.locale,
                    negative_stock_policy: request.negative_stock_policy,
                },
                &occurred_at,
            )
            .await?;
        Ok(OrganizationDto::from(&updated))
    }

    /// Lista las sucursales de la organización con su zona horaria efectiva.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `branch.read`.
    pub async fn branch_list(&self) -> Result<Vec<BranchAdminDto>, IpcError> {
        let principal = self.authenticated_principal().await?;
        let branches = self.administration()?.list_branches(&principal).await?;
        Ok(branches.iter().map(BranchAdminDto::from).collect())
    }

    /// Lee una sucursal concreta de la organización del principal.
    ///
    /// # Errors
    /// Devuelve [`IpcError::NotFound`] si no pertenece a la organización y
    /// [`IpcError::Forbidden`] sin `branch.read`.
    pub async fn branch_get(&self, branch_id: &str) -> Result<BranchAdminDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let branch_id = EntityId::parse(branch_id).map_err(|_| IpcError::ValidationError)?;
        let branch = self
            .administration()?
            .get_branch(&principal, &branch_id)
            .await?;
        Ok(BranchAdminDto::from(&branch))
    }

    /// Actualiza nombre y zona horaria de una sucursal existente.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `branch.update`,
    /// [`IpcError::ValidationError`] si el contrato de zona horaria es ambiguo,
    /// [`IpcError::NotFound`] si la sucursal pertenece a otra organización y
    /// [`IpcError::Conflict`] con una revisión obsoleta.
    pub async fn branch_update(
        &self,
        request: UpdateBranchRequestDto,
    ) -> Result<BranchAdminDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let branch_id =
            EntityId::parse(&request.branch_id).map_err(|_| IpcError::ValidationError)?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let updated = self
            .administration()?
            .update_branch(
                &principal,
                &mut identities,
                &branch_id,
                BranchConfigurationUpdate {
                    expected_revision: revision(&request.expected_revision)?,
                    name: request.name,
                    timezone: timezone_update(request.timezone)?,
                },
                &occurred_at,
            )
            .await?;
        Ok(BranchAdminDto::from(&updated))
    }

    /// Crea la sucursal inicial del negocio.
    ///
    /// # Errors
    /// Devuelve [`IpcError::Forbidden`] sin `branch.create`,
    /// [`IpcError::Conflict`] si ya existe una sucursal (capacidad de Free Local)
    /// o si el código está ocupado, y [`IpcError::ValidationError`] si los datos
    /// no cumplen el contrato.
    pub async fn branch_create(
        &self,
        request: CreateBranchRequestDto,
    ) -> Result<BranchAdminDto, IpcError> {
        let principal = self.authenticated_principal().await?;
        let (_, occurred_at) = crate::auth::clock(&self.product_installation()?.pool).await?;
        let mut identities = SecureIdentities;
        let created = self
            .administration()?
            .create_branch(
                &principal,
                &mut identities,
                BranchCreation {
                    code: request.code,
                    name: request.name,
                    timezone: request.timezone,
                },
                &occurred_at,
            )
            .await?;
        Ok(BranchAdminDto::from(&created))
    }

    fn administration(&self) -> Result<SqliteOrganizationAdministration, IpcError> {
        let product = self.product_installation()?;
        Ok(OrganizationAdministrationApplication::new(
            SqliteUnitOfWork::new(product.pool.clone()),
        ))
    }
}

/// Parser estricto de la revisión enviada como texto decimal.
fn revision(value: &str) -> Result<u64, IpcError> {
    value.parse::<u64>().map_err(|_| IpcError::ValidationError)
}

/// Traduce el triestado explícito del transporte al del dominio.
///
/// `KEEP`, `INHERIT` y `SET` son operaciones distintas: no se colapsan en un
/// `null` ambiguo.
fn timezone_update(request: TimezoneOverrideRequestDto) -> Result<TimezoneUpdate, IpcError> {
    match request.mode.as_str() {
        "KEEP" if request.timezone.is_none() => Ok(TimezoneUpdate::Keep),
        "INHERIT" if request.timezone.is_none() => Ok(TimezoneUpdate::Clear),
        "SET" => match request.timezone {
            Some(value) if !value.trim().is_empty() => Ok(TimezoneUpdate::Set(value)),
            _ => Err(IpcError::ValidationError),
        },
        _ => Err(IpcError::ValidationError),
    }
}

impl From<&Organization> for OrganizationDto {
    fn from(organization: &Organization) -> Self {
        Self {
            id: organization.id().as_str().into(),
            code: organization.code().as_str().into(),
            name: organization.name().as_str().into(),
            status: organization.status().as_str().into(),
            timezone: organization.timezone().as_str().into(),
            currency: organization.currency().as_str().into(),
            locale: organization.locale().as_str().into(),
            negative_stock_policy: organization.negative_stock_policy().as_str().into(),
            revision: organization.revision().to_string(),
        }
    }
}

impl From<&BranchConfiguration> for BranchAdminDto {
    fn from(configuration: &BranchConfiguration) -> Self {
        let branch = &configuration.branch;
        Self {
            id: branch.id().as_str().into(),
            code: branch.code().as_str().into(),
            name: branch.name().as_str().into(),
            status: branch.status().as_str().into(),
            timezone_override: branch
                .timezone_override()
                .map(|timezone| timezone.as_str().into()),
            effective_timezone: configuration.effective_timezone.as_str().into(),
            inherits_timezone: !configuration.overrides_timezone(),
            revision: branch.revision().to_string(),
        }
    }
}
