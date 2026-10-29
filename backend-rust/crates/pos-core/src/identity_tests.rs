// Fixtures deterministas para reglas puras; no representan secretos de producto.
#![allow(clippy::panic)]

use std::collections::BTreeSet;

use crate::{
    audit::{AuditAttributionError, AuditScope},
    identity::{
        CredentialKind, CredentialReference, CredentialState, IdentityError, LocalCredential,
        LocalIdentityRepository, LocalPrincipalService, LocalSession, LocalSessionState,
        NormalizedEmail, PermissionAuthority, PermissionCode, PosUser, PosUserPatch, PosUserStatus,
        SessionHandleHash, UtcSeconds,
    },
    organization_branch::{DisplayName, EntityId},
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture time: {error}"))
}
fn hash() -> SessionHandleHash {
    SessionHandleHash::new("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .unwrap_or_else(|error| panic!("fixture hash: {error}"))
}
fn user() -> PosUser {
    PosUser::create(
        id("00000000-0000-0000-0000-000000000101"),
        id("00000000-0000-0000-0000-000000000102"),
        DisplayName::new("Operadora Local").unwrap_or_else(|error| panic!("name: {error}")),
        NormalizedEmail::new(" Admin@Example.TEST ")
            .unwrap_or_else(|error| panic!("email: {error}")),
    )
}

#[test]
fn user_normalization_owner_and_patch_are_atomic() {
    let mut value = user();
    assert_eq!(value.email().as_str(), "admin@example.test");
    assert_eq!(
        NormalizedEmail::new("bad"),
        Err(IdentityError::InvalidEmail)
    );
    assert_eq!(
        NormalizedEmail::new("a@b@c.test"),
        Err(IdentityError::InvalidEmail)
    );
    assert!(
        value
            .patch(PosUserPatch {
                expected_revision: 0,
                display_name: Some(
                    DisplayName::new("Administradora")
                        .unwrap_or_else(|error| panic!("name: {error}"))
                ),
                email: None,
                status: None,
            })
            .unwrap_or_else(|error| panic!("patch: {error}"))
    );
    assert_eq!(value.revision(), 1);
    let before = value.clone();
    assert_eq!(
        value.patch(PosUserPatch {
            expected_revision: 0,
            display_name: None,
            email: Some(
                NormalizedEmail::new("other@example.test")
                    .unwrap_or_else(|error| panic!("email: {error}"))
            ),
            status: Some(PosUserStatus::Inactive),
        }),
        Err(IdentityError::ConcurrentModification)
    );
    assert_eq!(value, before);
}

#[test]
fn credential_is_redacted_owned_and_requires_reset_with_cas() {
    let owner = user();
    let mut credential = LocalCredential::create(
        id("00000000-0000-0000-0000-000000000103"),
        owner.id().clone(),
        CredentialKind::LegacyBcrypt,
        CredentialReference::new("$2b$12$synthetic-only-reference-not-a-real-secret")
            .unwrap_or_else(|error| panic!("reference: {error}")),
        "legacy-bcrypt-v1",
    )
    .unwrap_or_else(|error| panic!("credential: {error}"));
    assert_eq!(credential.user_id(), owner.id());
    assert_eq!(credential.state(), CredentialState::Ready);
    assert!(
        credential
            .require_reset(0)
            .unwrap_or_else(|error| panic!("reset: {error}"))
    );
    assert_eq!(credential.state(), CredentialState::ResetRequired);
    assert_eq!(credential.require_reset(1), Ok(false));
    assert!(!format!("{credential:?}").contains("synthetic-only"));
    assert_eq!(
        LocalCredential::create(
            id("00000000-0000-0000-0000-000000000108"),
            owner.id().clone(),
            CredentialKind::Argon2id,
            CredentialReference::new("$argon2id$synthetic")
                .unwrap_or_else(|error| panic!("reference: {error}")),
            "Argon2 Policy",
        ),
        Err(IdentityError::InvalidCredentialPolicy)
    );
}

#[test]
fn session_rejects_cross_owner_expiry_and_stale_revoke() {
    let owner = user();
    let mut session = LocalSession::create(
        id("00000000-0000-0000-0000-000000000104"),
        owner.id().clone(),
        owner.organization_id().clone(),
        hash(),
        moment(100),
        moment(200),
    )
    .unwrap_or_else(|error| panic!("session: {error}"));
    assert_eq!(session.state_at(moment(199)), LocalSessionState::Active);
    assert_eq!(session.state_at(moment(200)), LocalSessionState::Expired);
    assert_eq!(
        session.validate_for(
            &id("00000000-0000-0000-0000-000000000105"),
            owner.organization_id(),
            moment(101)
        ),
        Err(IdentityError::OwnershipMismatch)
    );
    assert_eq!(
        session.revoke(1, moment(150)),
        Err(IdentityError::ConcurrentModification)
    );
    assert!(
        session
            .revoke(0, moment(150))
            .unwrap_or_else(|error| panic!("revoke: {error}"))
    );
    assert_eq!(session.state_at(moment(151)), LocalSessionState::Revoked);
    assert_eq!(session.revision(), 1);
    assert_eq!(session.revoke(1, moment(160)), Ok(false));
    assert_eq!(session.revision(), 1);
    assert!(!format!("{session:?}").contains("012345"));
}

#[derive(Clone)]
struct Repository {
    session: Option<LocalSession>,
    user: Option<PosUser>,
}
impl LocalIdentityRepository for Repository {
    fn find_session_by_hash(
        &self,
        provided: &SessionHandleHash,
    ) -> Result<Option<LocalSession>, IdentityError> {
        Ok(self
            .session
            .as_ref()
            .filter(|session| session.handle_hash() == provided)
            .cloned())
    }
    fn find_user(
        &self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Option<PosUser>, IdentityError> {
        Ok(self
            .user
            .as_ref()
            .filter(|user| user.organization_id() == organization_id && user.id() == user_id)
            .cloned())
    }
}
struct Permissions(Result<BTreeSet<PermissionCode>, IdentityError>);
impl PermissionAuthority for Permissions {
    fn resolve_permissions(
        &self,
        _: &EntityId,
        _: &EntityId,
        _: Option<&EntityId>,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        self.0.clone()
    }
}

#[test]
fn db_authoritative_principal_requires_active_owned_session_and_permissions() {
    let owner = user();
    let session = LocalSession::create(
        id("00000000-0000-0000-0000-000000000106"),
        owner.id().clone(),
        owner.organization_id().clone(),
        hash(),
        moment(10),
        moment(20),
    )
    .unwrap_or_else(|error| panic!("session: {error}"));
    let mut grants = BTreeSet::new();
    grants.insert(
        PermissionCode::new("organization.update")
            .unwrap_or_else(|error| panic!("permission: {error}")),
    );
    let service = LocalPrincipalService::new(
        Repository {
            session: Some(session),
            user: Some(owner.clone()),
        },
        Permissions(Ok(grants)),
    );
    let principal = service
        .authenticate(&hash(), moment(11), None)
        .unwrap_or_else(|error| panic!("principal: {error}"));
    assert_eq!(principal.user_id(), owner.id());
    assert_eq!(principal.organization_id(), owner.organization_id());
    assert_eq!(principal.audit_actor().user_id(), Some(owner.id()));
    assert!(principal.permits_scope(&AuditScope {
        organization_id: owner.organization_id().clone(),
        branch_id: None,
    }));
    assert!(!principal.permits_scope(&AuditScope {
        organization_id: id("00000000-0000-0000-0000-000000000199"),
        branch_id: None,
    }));

    let expired = LocalPrincipalService::new(
        Repository {
            session: None,
            user: Some(owner),
        },
        Permissions(Ok(BTreeSet::new())),
    );
    assert_eq!(
        expired.authenticate(&hash(), moment(11), None),
        Err(AuditAttributionError::Unauthenticated)
    );
}

#[test]
fn inactive_user_and_permission_resolution_failure_fail_closed() {
    let mut inactive = user();
    inactive
        .patch(PosUserPatch {
            expected_revision: 0,
            display_name: None,
            email: None,
            status: Some(PosUserStatus::Inactive),
        })
        .unwrap_or_else(|error| panic!("patch: {error}"));
    let session = LocalSession::create(
        id("00000000-0000-0000-0000-000000000107"),
        inactive.id().clone(),
        inactive.organization_id().clone(),
        hash(),
        moment(1),
        moment(10),
    )
    .unwrap_or_else(|error| panic!("session: {error}"));
    let service = LocalPrincipalService::new(
        Repository {
            session: Some(session),
            user: Some(inactive),
        },
        Permissions(Ok(BTreeSet::new())),
    );
    assert_eq!(
        service.authenticate(&hash(), moment(2), None),
        Err(AuditAttributionError::Unauthenticated)
    );
}
