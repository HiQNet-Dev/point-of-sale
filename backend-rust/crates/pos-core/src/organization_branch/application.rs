use super::*;

/// Casos de uso de Organization sobre puertos transaccionales.
pub struct OrganizationApplication<U, A, I> {
    unit_of_work: U,
    authorization: A,
    identities: I,
}

impl<U, A, I> OrganizationApplication<U, A, I>
where
    U: UnitOfWork,
    A: Authorization,
    I: IdentityGenerator,
{
    /// Ensambla los puertos; no instala una política allow-all.
    pub fn new(unit_of_work: U, authorization: A, identities: I) -> Self {
        Self {
            unit_of_work,
            authorization,
            identities,
        }
    }

    /// Crea la organización exclusivamente para un contexto bootstrap autorizado.
    pub fn create(&mut self, input: NewOrganization) -> Result<Organization, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::BootstrapOrganization, None, None)
        {
            return Err(ApplicationError::Forbidden);
        }
        let id = self.identities.next_id()?;
        self.unit_of_work.transaction(|session| {
            if session.organization_code_exists(&input.code)? {
                return Err(ApplicationError::OrganizationCodeAlreadyExists);
            }
            let organization = Organization::create(id, input);
            session.insert_organization(&organization)?;
            Ok(organization)
        })
    }

    /// Recupera una organización tras autorizar su scope.
    pub fn get(&mut self, organization_id: &EntityId) -> Result<Organization, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::OrganizationRead, Some(organization_id), None)
        {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            session
                .get_organization(organization_id)?
                .ok_or(ApplicationError::OrganizationNotFound)
        })
    }

    /// Aplica un patch CAS y audita únicamente una mutación real, como Java.
    pub fn update(
        &mut self,
        organization_id: &EntityId,
        patch: OrganizationPatch,
    ) -> Result<Organization, ApplicationError> {
        if !self.authorization.is_allowed(
            Permission::OrganizationUpdate,
            Some(organization_id),
            None,
        ) {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            let mut organization = session
                .get_organization(organization_id)?
                .ok_or(ApplicationError::OrganizationNotFound)?;
            let before = organization.clone();
            let previous_revision = organization.revision();
            let changed = organization.patch(patch)?;
            if !changed {
                return Ok(organization);
            }
            if !session.update_organization_cas(previous_revision, &organization)? {
                return Err(ApplicationError::ConcurrentModification);
            }
            session.append_audit(AuditEvent {
                organization_id: organization_id.clone(),
                branch_id: None,
                action: "organization.updated",
                entity_type: "organization",
                entity_id: organization_id.clone(),
                changed_fields: organization_changed_fields(&before, &organization),
            })?;
            Ok(organization)
        })
    }

    /// Descompone el servicio una vez configurado por el composition root.
    pub fn into_parts(self) -> (U, A, I) {
        (self.unit_of_work, self.authorization, self.identities)
    }
}

/// Casos de uso de Branch sobre los mismos puertos transaccionales.
pub struct BranchApplication<U, A, I> {
    unit_of_work: U,
    authorization: A,
    identities: I,
}

impl<U, A, I> BranchApplication<U, A, I>
where
    U: UnitOfWork,
    A: Authorization,
    I: IdentityGenerator,
{
    /// Ensambla el caso de uso sin abrir ninguna autorización por defecto.
    pub fn new(unit_of_work: U, authorization: A, identities: I) -> Self {
        Self {
            unit_of_work,
            authorization,
            identities,
        }
    }

    /// Crea y audita una sucursal dentro de una organización existente.
    pub fn create(
        &mut self,
        organization_id: &EntityId,
        input: NewBranch,
    ) -> Result<Branch, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::BranchCreate, Some(organization_id), None)
        {
            return Err(ApplicationError::Forbidden);
        }
        let id = self.identities.next_id()?;
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            if session.branch_code_exists(organization_id, &input.code)? {
                return Err(ApplicationError::BranchCodeAlreadyExists);
            }
            let branch = Branch::create(id, organization_id.clone(), input);
            session.insert_branch(&branch)?;
            session.append_audit(AuditEvent {
                organization_id: organization_id.clone(),
                branch_id: Some(branch.id().clone()),
                action: "branch.created",
                entity_type: "branch",
                entity_id: branch.id().clone(),
                changed_fields: Vec::new(),
            })?;
            Ok(branch)
        })
    }

    /// Lista sucursales únicamente dentro del owner visible.
    pub fn list(&mut self, organization_id: &EntityId) -> Result<Vec<Branch>, ApplicationError> {
        if !self
            .authorization
            .is_allowed(Permission::BranchRead, Some(organization_id), None)
        {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            session.list_branches(organization_id)
        })
    }

    /// Obtiene una sucursal scoped, ocultando IDs de otras organizaciones.
    pub fn get(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<Branch, ApplicationError> {
        if !self.authorization.is_allowed(
            Permission::BranchRead,
            Some(organization_id),
            Some(branch_id),
        ) {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            session
                .get_branch(organization_id, branch_id)?
                .ok_or(ApplicationError::BranchNotFound)
        })
    }

    /// Descompone el servicio una vez configurado por el composition root.
    pub fn into_parts(self) -> (U, A, I) {
        (self.unit_of_work, self.authorization, self.identities)
    }

    /// Aplica CAS scoped y registra solo cambios materiales.
    pub fn update(
        &mut self,
        organization_id: &EntityId,
        branch_id: &EntityId,
        patch: BranchPatch,
    ) -> Result<Branch, ApplicationError> {
        if !self.authorization.is_allowed(
            Permission::BranchUpdate,
            Some(organization_id),
            Some(branch_id),
        ) {
            return Err(ApplicationError::Forbidden);
        }
        self.unit_of_work.transaction(|session| {
            if session.get_organization(organization_id)?.is_none() {
                return Err(ApplicationError::OrganizationNotFound);
            }
            let mut branch = session
                .get_branch(organization_id, branch_id)?
                .ok_or(ApplicationError::BranchNotFound)?;
            let before = branch.clone();
            let previous_revision = branch.revision();
            let changed = branch.patch(organization_id, patch)?;
            if !changed {
                return Ok(branch);
            }
            if !session.update_branch_cas(organization_id, previous_revision, &branch)? {
                return Err(ApplicationError::ConcurrentModification);
            }
            session.append_audit(AuditEvent {
                organization_id: organization_id.clone(),
                branch_id: Some(branch_id.clone()),
                action: "branch.updated",
                entity_type: "branch",
                entity_id: branch_id.clone(),
                changed_fields: branch_changed_fields(&before, &branch),
            })?;
            Ok(branch)
        })
    }
}
