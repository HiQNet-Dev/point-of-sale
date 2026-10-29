use std::collections::VecDeque;

use crate::organization_branch::{
    ApplicationError, AuditAppender, AuditEvent, Authorization, Branch, BranchApplication, BranchPatch,
    BranchRepository, BusinessCode, DisplayName, DomainError, EntityId, IdentityGenerator, IsoCurrency,
    LocaleTag, NegativeStockPolicy, NewBranch, NewOrganization, Organization, OrganizationApplication,
    OrganizationPatch, OrganizationRepository, Permission, Status, Timezone,
    TimezoneOverridePatch, UnitOfWork,
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

fn organization_input(code: &str) -> NewOrganization {
    NewOrganization {
        name: DisplayName::new(" Organización principal ").unwrap_or_else(|error| panic!("name: {error}")),
        code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
        timezone: Timezone::new("America/Mexico_City").unwrap_or_else(|error| panic!("timezone: {error}")),
        currency: IsoCurrency::new("mxn").unwrap_or_else(|error| panic!("currency: {error}")),
        locale: LocaleTag::new("es-MX-u-ca-gregory").unwrap_or_else(|error| panic!("locale: {error}")),
    }
}

fn branch_input(code: &str) -> NewBranch {
    NewBranch {
        name: DisplayName::new(" Sucursal Centro ").unwrap_or_else(|error| panic!("name: {error}")),
        code: BusinessCode::new(code).unwrap_or_else(|error| panic!("code: {error}")),
        timezone_override: None,
    }
}

#[test]
fn java_normalization_and_utf16_limits_are_preserved_without_panics() {
    assert_eq!(BusinessCode::new("\u{0009}MAIN-OFFICE\u{0020}").map(|code| code.as_str().to_owned()), Ok("main-office".to_owned()));
    assert_eq!(BusinessCode::new(" oficina "), Err(DomainError::InvalidCode));
    assert_eq!(DisplayName::new("\u{00a0}x\u{00a0}").map(|name| name.as_str().to_owned()), Ok("\u{00a0}x\u{00a0}".to_owned()));
    assert_eq!(DisplayName::new(&"😀".repeat(60)), Ok(DisplayName::new(&"😀".repeat(60)).unwrap_or_else(|error| panic!("name: {error}"))));
    assert_eq!(DisplayName::new(&"😀".repeat(61)), Err(DomainError::InvalidName));
    for malformed in ["\0", "é", "😀", "--", "office--one"] {
        assert!(BusinessCode::new(malformed).is_err());
    }
}

#[test]
fn timezone_currency_and_locale_cover_java_compatible_vectors() {
    for zone in ["America/Mexico_City", "Etc/GMT+5", "Z", "UTC", "GMT", "UT", "+18:00", "-1800", "UTC+05:30"] {
        assert!(Timezone::new(zone).is_ok(), "{zone}");
    }
    for zone in ["+18:01", "-18:01", "+19:00", "UTC+18:01", "Europe/", "America//Mexico", " "] {
        assert!(Timezone::new(zone).is_err(), "{zone}");
    }
    for currency in ["USD", "MXN", "eur", "XTS"] {
        assert!(IsoCurrency::new(currency).is_ok(), "{currency}");
    }
    for currency in ["ZZZ", "EU", "€UR", "usd "] {
        assert_eq!(IsoCurrency::new(currency), Err(DomainError::InvalidCurrency));
    }
    for locale in ["es-MX", "en-US", "es-MX-u-ca-gregory", "zh-Hant-TW"] {
        assert!(LocaleTag::new(locale).is_ok(), "{locale}");
    }
    for locale in ["", "es_MX", "e", "es--MX", "es-こんにちは"] {
        assert!(LocaleTag::new(locale).is_err(), "{locale}");
    }
}

#[test]
fn organization_patch_is_atomic_and_noop_keeps_revision() {
    let mut organization = Organization::create(id("00000000-0000-0000-0000-000000000001"), organization_input("main"));
    let before = organization.clone();
    assert_eq!(organization.patch(OrganizationPatch {
        expected_revision: 1,
        name: None,
        timezone: None,
        currency: None,
        locale: None,
        status: None,
        negative_stock_policy: None,
    }), Err(DomainError::StaleRevision));
    assert_eq!(organization, before);
    assert_eq!(organization.patch(OrganizationPatch {
        expected_revision: 0,
        name: Some(DisplayName::new("Organización principal").unwrap_or_else(|error| panic!("name: {error}"))),
        timezone: None,
        currency: None,
        locale: None,
        status: None,
        negative_stock_policy: None,
    }), Ok(false));
    assert_eq!(organization.revision(), 0);
    assert_eq!(organization.patch(OrganizationPatch {
        expected_revision: 0,
        name: None,
        timezone: None,
        currency: None,
        locale: None,
        status: Some(Status::Inactive),
        negative_stock_policy: Some(NegativeStockPolicy::AllowNegative),
    }), Ok(true));
    assert_eq!(organization.revision(), 1);
    assert_eq!(organization.code().as_str(), "main");
}

#[test]
fn branch_owner_timezone_inheritance_and_clear_are_atomic() {
    let owner = id("00000000-0000-0000-0000-000000000001");
    let other_owner = id("00000000-0000-0000-0000-000000000002");
    let parent_timezone = Timezone::new("UTC").unwrap_or_else(|error| panic!("timezone: {error}"));
    let mut branch = Branch::create(id("00000000-0000-0000-0000-000000000003"), owner.clone(), branch_input("center"));
    assert_eq!(branch.effective_timezone(&parent_timezone).as_str(), "UTC");
    assert_eq!(branch.patch(&owner, BranchPatch { expected_revision: 0, name: None, timezone_override: TimezoneOverridePatch::Set(Timezone::new("America/New_York").unwrap_or_else(|error| panic!("timezone: {error}"))), status: None }), Ok(true));
    assert_eq!(branch.effective_timezone(&parent_timezone).as_str(), "America/New_York");
    assert_eq!(branch.patch(&owner, BranchPatch { expected_revision: 1, name: None, timezone_override: TimezoneOverridePatch::Clear, status: None }), Ok(true));
    assert_eq!(branch.effective_timezone(&parent_timezone).as_str(), "UTC");
    let before = branch.clone();
    assert_eq!(branch.patch(&other_owner, BranchPatch { expected_revision: 2, name: None, timezone_override: TimezoneOverridePatch::Keep, status: None }), Err(DomainError::OwnershipMismatch));
    assert_eq!(branch, before);
}

#[derive(Clone, Default)]
struct MemorySession {
    organizations: Vec<Organization>,
    branches: Vec<Branch>,
    audit: Vec<AuditEvent>,
    fail_audit: bool,
}

impl OrganizationRepository for MemorySession {
    fn get_organization(&mut self, wanted: &EntityId) -> Result<Option<Organization>, ApplicationError> {
        Ok(self.organizations.iter().find(|organization| organization.id() == wanted).cloned())
    }
    fn organization_code_exists(&mut self, code: &BusinessCode) -> Result<bool, ApplicationError> {
        Ok(self.organizations.iter().any(|organization| organization.code() == code))
    }
    fn insert_organization(&mut self, organization: &Organization) -> Result<(), ApplicationError> {
        if self.organization_code_exists(organization.code())? { return Err(ApplicationError::OrganizationCodeAlreadyExists); }
        self.organizations.push(organization.clone());
        Ok(())
    }
    fn update_organization_cas(&mut self, previous_revision: u64, organization: &Organization) -> Result<bool, ApplicationError> {
        let Some(current) = self.organizations.iter_mut().find(|current| current.id() == organization.id()) else { return Ok(false); };
        if current.revision() != previous_revision { return Ok(false); }
        *current = organization.clone();
        Ok(true)
    }
}

impl BranchRepository for MemorySession {
    fn get_branch(&mut self, owner: &EntityId, wanted: &EntityId) -> Result<Option<Branch>, ApplicationError> {
        Ok(self.branches.iter().find(|branch| branch.organization_id() == owner && branch.id() == wanted).cloned())
    }
    fn list_branches(&mut self, owner: &EntityId) -> Result<Vec<Branch>, ApplicationError> {
        Ok(self.branches.iter().filter(|branch| branch.organization_id() == owner).cloned().collect())
    }
    fn branch_code_exists(&mut self, owner: &EntityId, code: &BusinessCode) -> Result<bool, ApplicationError> {
        Ok(self.branches.iter().any(|branch| branch.organization_id() == owner && branch.code() == code))
    }
    fn insert_branch(&mut self, branch: &Branch) -> Result<(), ApplicationError> {
        if self.branch_code_exists(branch.organization_id(), branch.code())? { return Err(ApplicationError::BranchCodeAlreadyExists); }
        self.branches.push(branch.clone());
        Ok(())
    }
    fn update_branch_cas(&mut self, owner: &EntityId, previous_revision: u64, branch: &Branch) -> Result<bool, ApplicationError> {
        let Some(current) = self.branches.iter_mut().find(|current| current.organization_id() == owner && current.id() == branch.id()) else { return Ok(false); };
        if current.revision() != previous_revision { return Ok(false); }
        *current = branch.clone();
        Ok(true)
    }
}

impl AuditAppender for MemorySession {
    fn append_audit(&mut self, event: AuditEvent) -> Result<(), ApplicationError> {
        if self.fail_audit { return Err(ApplicationError::Audit); }
        self.audit.push(event);
        Ok(())
    }
}

struct MemoryUow(MemorySession);
impl UnitOfWork for MemoryUow {
    type Session = MemorySession;
    fn transaction<T>(&mut self, operation: impl FnOnce(&mut Self::Session) -> Result<T, ApplicationError>) -> Result<T, ApplicationError> {
        let mut candidate = self.0.clone();
        let output = operation(&mut candidate)?;
        self.0 = candidate;
        Ok(output)
    }
}

struct FixedAuthorization(bool);
impl Authorization for FixedAuthorization {
    fn is_allowed(&self, _: Permission, _: Option<&EntityId>, _: Option<&EntityId>) -> bool { self.0 }
}

struct FixedIds(VecDeque<EntityId>);
impl IdentityGenerator for FixedIds {
    fn next_id(&mut self) -> Result<EntityId, ApplicationError> { self.0.pop_front().ok_or(ApplicationError::Storage) }
}

#[test]
fn application_enforces_authorization_duplicate_scope_and_audit_rollback() {
    let organization_id = id("00000000-0000-0000-0000-000000000010");
    let branch_id = id("00000000-0000-0000-0000-000000000011");
    let ids = FixedIds(VecDeque::from([
        organization_id.clone(),
        id("00000000-0000-0000-0000-000000000099"),
        branch_id.clone(),
        id("00000000-0000-0000-0000-000000000098"),
    ]));
    let mut organizations = OrganizationApplication::new(MemoryUow(MemorySession::default()), FixedAuthorization(true), ids);
    let organization = organizations.create(organization_input("main")).unwrap_or_else(|error| panic!("create organization: {error:?}"));
    assert_eq!(organization.id(), &organization_id);
    assert_eq!(organizations.create(organization_input(" MAIN ")), Err(ApplicationError::OrganizationCodeAlreadyExists));
    let (uow, _, ids) = organizations.into_parts();
    let mut branches = BranchApplication::new(MemoryUow(uow.0), FixedAuthorization(true), ids);
    let branch = branches.create(&organization_id, branch_input("center")).unwrap_or_else(|error| panic!("create branch: {error:?}"));
    assert_eq!(branch.id(), &branch_id);
    assert_eq!(branches.create(&organization_id, branch_input(" CENTER ")), Err(ApplicationError::BranchCodeAlreadyExists));
    let foreign = id("00000000-0000-0000-0000-000000000012");
    assert_eq!(branches.get(&foreign, &branch_id), Err(ApplicationError::OrganizationNotFound));
    let updated = branches.update(&organization_id, &branch_id, BranchPatch { expected_revision: 0, name: Some(DisplayName::new("Otro nombre").unwrap_or_else(|error| panic!("name: {error}"))), timezone_override: TimezoneOverridePatch::Keep, status: None }).unwrap_or_else(|error| panic!("update branch: {error:?}"));
    assert_eq!(updated.revision(), 1);
    let (mut uow, _, _) = branches.into_parts();
    assert_eq!(uow.0.audit[1].action, "branch.updated");
    assert_eq!(uow.0.audit[1].changed_fields, vec!["name"]);
    uow.0.fail_audit = true;
    let mut branches = BranchApplication::new(uow, FixedAuthorization(true), FixedIds(VecDeque::new()));
    assert_eq!(branches.update(&organization_id, &branch_id, BranchPatch { expected_revision: 1, name: None, timezone_override: TimezoneOverridePatch::Keep, status: Some(Status::Inactive) }), Err(ApplicationError::Audit));
    let (uow, _, _) = branches.into_parts();
    assert_eq!(uow.0.branches[0].name().as_str(), "Otro nombre");
    assert_eq!(uow.0.branches[0].revision(), 1);
    assert_eq!(uow.0.audit.len(), 2);
}

#[test]
fn application_denies_without_an_explicit_authorizer_grant() {
    let mut application = OrganizationApplication::new(
        MemoryUow(MemorySession::default()),
        FixedAuthorization(false),
        FixedIds(VecDeque::from([id("00000000-0000-0000-0000-000000000010")])),
    );
    assert_eq!(application.create(organization_input("main")), Err(ApplicationError::Forbidden));
}
