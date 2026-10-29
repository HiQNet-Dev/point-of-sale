// Fixtures deterministas para reglas puras; no representan secretos de producto.
#![allow(clippy::panic)]

use std::collections::BTreeSet;

use serde_json::json;

use crate::{
    audit::AuditAttributionError,
    identity::{
        AuthorizationError, CredentialAlgorithm, CredentialKind, CredentialParameters,
        CredentialState, IdentityError, LocalCredential, LocalIdentityRepository,
        LocalPrincipalService, LocalSession, LocalSessionState, LockoutPolicy, NewCredential,
        NormalizedEmail, PermissionAuthority, PermissionCode, PosUser, PosUserPatch, PosUserStatus,
        PrincipalScope, SecretHash, SessionHash, UtcSeconds,
    },
    organization_branch::{DisplayName, EntityId},
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}
fn moment(value: i64) -> UtcSeconds {
    UtcSeconds::new(value).unwrap_or_else(|error| panic!("fixture time: {error}"))
}
fn hash() -> SessionHash {
    SessionHash::new("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
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
fn secret() -> SecretHash {
    SecretHash::new("$2b$12$synthetic-only-reference-not-a-real-secret")
        .unwrap_or_else(|error| panic!("secret: {error}"))
}
fn credential() -> LocalCredential {
    LocalCredential::create(NewCredential {
        id: id("00000000-0000-0000-0000-000000000103"),
        user_id: id("00000000-0000-0000-0000-000000000101"),
        kind: CredentialKind::Password,
        algorithm: CredentialAlgorithm::LegacyBcrypt,
        secret_hash: secret(),
        parameters: CredentialParameters::new(&json!({ "cost": 12 }))
            .unwrap_or_else(|error| panic!("parameters: {error}")),
        policy_id: "legacy-bcrypt-v1".to_owned(),
        created_at: moment(10),
    })
    .unwrap_or_else(|error| panic!("credential: {error}"))
}
fn permission(value: &str) -> PermissionCode {
    PermissionCode::new(value).unwrap_or_else(|error| panic!("permission: {error}"))
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
                expected_version: 0,
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
            expected_version: 0,
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
fn credential_separates_purpose_algorithm_policy_and_redacts_material() {
    let value = credential();
    assert_eq!(value.kind(), CredentialKind::Password);
    assert_eq!(value.algorithm(), CredentialAlgorithm::LegacyBcrypt);
    assert_eq!(value.state(), CredentialState::Active);
    assert_eq!(value.credential_version(), 1);
    assert_eq!(value.failed_login_attempts(), 0);
    assert_eq!(value.locked_until(), None);
    assert!(!format!("{value:?}").contains("synthetic-only"));
    assert_eq!(
        SecretHash::new("line\nbreak"),
        Err(IdentityError::InvalidSecretHash)
    );
    let invalid_policy = LocalCredential::create(NewCredential {
        id: id("00000000-0000-0000-0000-000000000108"),
        user_id: id("00000000-0000-0000-0000-000000000101"),
        kind: CredentialKind::Pin,
        algorithm: CredentialAlgorithm::Argon2id,
        secret_hash: SecretHash::new("$argon2id$synthetic")
            .unwrap_or_else(|error| panic!("secret: {error}")),
        parameters: CredentialParameters::new(&json!({ "m_cost": 65536 }))
            .unwrap_or_else(|error| panic!("parameters: {error}")),
        policy_id: "Argon2 Policy".to_owned(),
        created_at: moment(10),
    });
    assert_eq!(invalid_policy, Err(IdentityError::InvalidCredentialPolicy));
}

#[test]
fn credential_parameters_reject_arrays_secrets_depth_and_oversize() {
    assert_eq!(
        CredentialParameters::new(&json!([1, 2])),
        Err(IdentityError::InvalidCredentialParameters)
    );
    assert_eq!(
        CredentialParameters::new(&json!({})),
        Err(IdentityError::InvalidCredentialParameters)
    );
    for forbidden in ["password", "Secret_Value", "api-token", "credentialSalt"] {
        assert_eq!(
            CredentialParameters::new(&json!({ "outer": { forbidden: "x" } })),
            Err(IdentityError::InvalidCredentialParameters),
            "clave sensible aceptada: {forbidden}"
        );
    }
    let deep = json!({ "a": { "b": { "c": { "d": { "e": 1 } } } } });
    assert_eq!(
        CredentialParameters::new(&deep),
        Err(IdentityError::InvalidCredentialParameters)
    );
    let oversized = json!({ "note": "x".repeat(2_048) });
    assert_eq!(
        CredentialParameters::new(&oversized),
        Err(IdentityError::InvalidCredentialParameters)
    );
    let valid = CredentialParameters::new(&json!({ "m_cost": 65536, "t_cost": 3, "p_cost": 1 }))
        .unwrap_or_else(|error| panic!("parameters: {error}"));
    assert_eq!(
        valid.canonical_json(),
        "{\"m_cost\":65536,\"p_cost\":1,\"t_cost\":3}"
    );
}

#[test]
fn credential_lockout_matches_java_failure_window_and_clears_on_success() {
    let mut value = credential();
    let policy = LockoutPolicy::new(3, 60).unwrap_or_else(|error| panic!("policy: {error}"));
    assert!(
        !value
            .record_failed_login(moment(10), policy, 1)
            .unwrap_or_else(|error| panic!("failure: {error}"))
    );
    assert_eq!(value.failed_login_attempts(), 1);
    assert_eq!(value.credential_version(), 2);
    value
        .record_failed_login(moment(11), policy, 2)
        .unwrap_or_else(|error| panic!("failure: {error}"));
    assert!(
        value
            .record_failed_login(moment(12), policy, 3)
            .unwrap_or_else(|error| panic!("failure: {error}"))
    );
    assert_eq!(value.credential_version(), 4);
    assert!(value.is_locked(moment(71)));
    assert!(!value.is_locked(moment(72)));
    assert_eq!(
        value.accepts_verification(moment(71)),
        Err(IdentityError::InvalidCredentialState)
    );
    // Ventana vencida: el contador se reinicia como en `User.recordFailedLogin`.
    assert!(
        !value
            .record_failed_login(moment(73), policy, 4)
            .unwrap_or_else(|error| panic!("failure: {error}"))
    );
    assert_eq!(value.failed_login_attempts(), 1);
    assert_eq!(value.locked_until(), None);
    assert!(
        value
            .record_successful_login(moment(74), 5)
            .unwrap_or_else(|error| panic!("success: {error}"))
    );
    assert_eq!(value.failed_login_attempts(), 0);
    assert_eq!(value.credential_version(), 6);
    assert!(
        !value
            .record_successful_login(moment(75), 6)
            .unwrap_or_else(|error| panic!("success: {error}"))
    );
    assert_eq!(
        LockoutPolicy::new(0, 60),
        Err(IdentityError::InvalidLockoutPolicy)
    );
    assert_eq!(
        LockoutPolicy::new(3, 0),
        Err(IdentityError::InvalidLockoutPolicy)
    );
}

#[test]
fn credential_state_transitions_reject_incompatible_operations_atomically() {
    let mut value = credential();
    value
        .require_reset(1)
        .unwrap_or_else(|error| panic!("reset: {error}"));
    assert_eq!(value.state(), CredentialState::ResetRequired);
    assert_eq!(value.require_reset(2), Ok(false));
    assert_eq!(
        value.record_successful_login(moment(20), 2),
        Err(IdentityError::InvalidCredentialState)
    );
    assert!(
        value
            .disable(2)
            .unwrap_or_else(|error| panic!("disable: {error}"))
    );
    assert_eq!(value.state(), CredentialState::Disabled);
    let before = value.clone();
    assert_eq!(
        value.require_reset(3),
        Err(IdentityError::InvalidCredentialState)
    );
    assert_eq!(
        value.record_failed_login(
            moment(21),
            LockoutPolicy::new(3, 60).unwrap_or_else(|error| panic!("policy: {error}")),
            3
        ),
        Err(IdentityError::InvalidCredentialState)
    );
    assert_eq!(
        value.rotate_material(
            CredentialAlgorithm::Argon2id,
            SecretHash::new("$argon2id$other").unwrap_or_else(|error| panic!("secret: {error}")),
            CredentialParameters::new(&json!({ "m_cost": 65536 }))
                .unwrap_or_else(|error| panic!("parameters: {error}")),
            "argon2id-v1",
            moment(22),
            3
        ),
        Err(IdentityError::InvalidCredentialState)
    );
    assert_eq!(value, before);
    assert_eq!(
        value.require_reset(1),
        Err(IdentityError::ConcurrentModification)
    );
    assert_eq!(value, before);
}

#[test]
fn credential_rotation_upgrades_algorithm_and_resets_failure_state() {
    let mut value = credential();
    value
        .record_failed_login(
            moment(10),
            LockoutPolicy::new(2, 60).unwrap_or_else(|error| panic!("policy: {error}")),
            1,
        )
        .unwrap_or_else(|error| panic!("failure: {error}"));
    assert!(
        value
            .rotate_material(
                CredentialAlgorithm::Argon2id,
                SecretHash::new("$argon2id$v=19$m=65536,t=3,p=1$c2FsdA$aGFzaA")
                    .unwrap_or_else(|error| panic!("secret: {error}")),
                CredentialParameters::new(&json!({ "m_cost": 65536, "t_cost": 3, "p_cost": 1 }))
                    .unwrap_or_else(|error| panic!("parameters: {error}")),
                "argon2id-v1",
                moment(11),
                2
            )
            .unwrap_or_else(|error| panic!("rotate: {error}"))
    );
    assert_eq!(value.algorithm(), CredentialAlgorithm::Argon2id);
    assert_eq!(value.state(), CredentialState::Active);
    assert_eq!(value.failed_login_attempts(), 0);
    assert_eq!(value.locked_until(), None);
    assert_eq!(value.credential_version(), 3);
    assert_eq!(value.policy_id(), "argon2id-v1");
    assert_eq!(
        value.rotate_material(
            CredentialAlgorithm::LegacyBcrypt,
            secret(),
            CredentialParameters::new(&json!({ "cost": 12 }))
                .unwrap_or_else(|error| panic!("parameters: {error}")),
            "legacy-bcrypt-v1",
            moment(12),
            2
        ),
        Err(IdentityError::ConcurrentModification)
    );
}

#[test]
fn session_alignment_version_ownership_and_revoke_are_enforced() {
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
    assert_eq!(session.session_version(), 1);
    assert_eq!(session.state_at(moment(199)), LocalSessionState::Active);
    assert_eq!(session.state_at(moment(200)), LocalSessionState::Expired);
    session
        .ensure_owned_by(&owner)
        .unwrap_or_else(|error| panic!("owner: {error}"));
    let other = PosUser::create(
        id("00000000-0000-0000-0000-000000000105"),
        owner.organization_id().clone(),
        DisplayName::new("Otra persona").unwrap_or_else(|error| panic!("name: {error}")),
        NormalizedEmail::new("other@example.test").unwrap_or_else(|error| panic!("email: {error}")),
    );
    assert_eq!(
        session.ensure_owned_by(&other),
        Err(IdentityError::OwnershipMismatch)
    );
    assert_eq!(
        session.validate_for(
            owner.id(),
            &id("00000000-0000-0000-0000-000000000199"),
            moment(101)
        ),
        Err(IdentityError::OwnershipMismatch)
    );
    assert_eq!(
        session.validate_for(owner.id(), owner.organization_id(), moment(200)),
        Err(IdentityError::SessionNotActive)
    );
    assert_eq!(
        session.revoke(2, moment(150)),
        Err(IdentityError::ConcurrentModification)
    );
    assert!(
        session
            .revoke(1, moment(150))
            .unwrap_or_else(|error| panic!("revoke: {error}"))
    );
    assert_eq!(session.session_version(), 2);
    assert_eq!(session.state_at(moment(151)), LocalSessionState::Revoked);
    assert_eq!(session.revoke(2, moment(160)), Ok(false));
    assert_eq!(session.session_version(), 2);
    assert!(!format!("{session:?}").contains("012345"));
}

#[derive(Clone)]
struct Repository {
    session: Option<LocalSession>,
    user: Option<PosUser>,
    fail: bool,
}
impl LocalIdentityRepository for Repository {
    fn find_session_by_hash(
        &self,
        provided: &SessionHash,
    ) -> Result<Option<LocalSession>, IdentityError> {
        if self.fail {
            return Err(IdentityError::InvalidSessionHash);
        }
        Ok(self
            .session
            .as_ref()
            .filter(|session| session.session_hash() == provided)
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

fn active_session(owner: &PosUser) -> LocalSession {
    LocalSession::create(
        id("00000000-0000-0000-0000-000000000106"),
        owner.id().clone(),
        owner.organization_id().clone(),
        hash(),
        moment(10),
        moment(1_000),
    )
    .unwrap_or_else(|error| panic!("session: {error}"))
}

fn grants(codes: &[&str]) -> BTreeSet<PermissionCode> {
    codes.iter().map(|code| permission(code)).collect()
}

#[test]
fn principal_retains_branch_scope_and_denies_other_scopes() {
    let owner = user();
    let branch_a = id("00000000-0000-0000-0000-000000000201");
    let branch_b = id("00000000-0000-0000-0000-000000000202");
    let service = LocalPrincipalService::new(
        Repository {
            session: Some(active_session(&owner)),
            user: Some(owner.clone()),
            fail: false,
        },
        Permissions(Ok(grants(&["branch.update"]))),
    );
    let principal = service
        .authenticate(&hash(), moment(11), Some(&branch_a))
        .unwrap_or_else(|error| panic!("principal: {error}"));
    assert_eq!(principal.scope().organization_id(), owner.organization_id());
    assert_eq!(principal.scope().branch_id(), Some(&branch_a));
    let scope_a = PrincipalScope::new(owner.organization_id().clone(), Some(branch_a.clone()));
    let scope_b = PrincipalScope::new(owner.organization_id().clone(), Some(branch_b));
    let scope_org = PrincipalScope::new(owner.organization_id().clone(), None);
    let update = permission("branch.update");
    assert_eq!(principal.authorize(&update, &scope_a), Ok(()));
    assert_eq!(
        principal.authorize(&update, &scope_b),
        Err(AuthorizationError::ScopeMismatch)
    );
    assert_eq!(
        principal.authorize(&update, &scope_org),
        Err(AuthorizationError::ScopeMismatch)
    );
    assert_eq!(
        principal.authorize(&permission("organization.update"), &scope_a),
        Err(AuthorizationError::PermissionDenied)
    );
    assert_eq!(
        principal.authorize(
            &update,
            &PrincipalScope::new(id("00000000-0000-0000-0000-000000000199"), Some(branch_a))
        ),
        Err(AuthorizationError::ScopeMismatch)
    );

    let org_service = LocalPrincipalService::new(
        Repository {
            session: Some(active_session(&owner)),
            user: Some(owner.clone()),
            fail: false,
        },
        Permissions(Ok(grants(&["organization.update"]))),
    );
    let org_principal = org_service
        .authenticate(&hash(), moment(11), None)
        .unwrap_or_else(|error| panic!("principal: {error}"));
    assert_eq!(
        org_principal.authorize(&permission("organization.update"), &scope_org),
        Ok(())
    );
    assert_eq!(
        org_principal.authorize(&permission("organization.update"), &scope_a),
        Err(AuthorizationError::ScopeMismatch)
    );
}

#[test]
fn principal_fails_closed_for_unknown_inactive_expired_and_authority_errors() {
    let owner = user();
    let missing = LocalPrincipalService::new(
        Repository {
            session: None,
            user: Some(owner.clone()),
            fail: false,
        },
        Permissions(Ok(BTreeSet::new())),
    );
    assert_eq!(
        missing.authenticate(&hash(), moment(11), None),
        Err(AuditAttributionError::Unauthenticated)
    );

    let mut inactive = owner.clone();
    inactive
        .patch(PosUserPatch {
            expected_version: 0,
            display_name: None,
            email: None,
            status: Some(PosUserStatus::Inactive),
        })
        .unwrap_or_else(|error| panic!("patch: {error}"));
    let disabled = LocalPrincipalService::new(
        Repository {
            session: Some(active_session(&inactive)),
            user: Some(inactive),
            fail: false,
        },
        Permissions(Ok(BTreeSet::new())),
    );
    assert_eq!(
        disabled.authenticate(&hash(), moment(11), None),
        Err(AuditAttributionError::Unauthenticated)
    );

    let expired = LocalPrincipalService::new(
        Repository {
            session: Some(active_session(&owner)),
            user: Some(owner.clone()),
            fail: false,
        },
        Permissions(Ok(BTreeSet::new())),
    );
    assert_eq!(
        expired.authenticate(&hash(), moment(1_001), None),
        Err(AuditAttributionError::Unauthenticated)
    );

    let storage_error = LocalPrincipalService::new(
        Repository {
            session: None,
            user: None,
            fail: true,
        },
        Permissions(Ok(BTreeSet::new())),
    );
    assert_eq!(
        storage_error.authenticate(&hash(), moment(11), None),
        Err(AuditAttributionError::UnattributablePrincipal)
    );

    let authority_error = LocalPrincipalService::new(
        Repository {
            session: Some(active_session(&owner)),
            user: Some(owner.clone()),
            fail: false,
        },
        Permissions(Err(IdentityError::InvalidPermission)),
    );
    assert_eq!(
        authority_error.authenticate(&hash(), moment(11), None),
        Err(AuditAttributionError::ScopeDenied)
    );
}

#[test]
fn revoked_session_cannot_build_a_principal_or_an_actor() {
    let owner = user();
    let mut session = active_session(&owner);
    session
        .revoke(1, moment(20))
        .unwrap_or_else(|error| panic!("revoke: {error}"));
    let service = LocalPrincipalService::new(
        Repository {
            session: Some(session),
            user: Some(owner.clone()),
            fail: false,
        },
        Permissions(Ok(grants(&["organization.update"]))),
    );
    assert_eq!(
        service.authenticate(&hash(), moment(21), None),
        Err(AuditAttributionError::Unauthenticated)
    );

    let live = LocalPrincipalService::new(
        Repository {
            session: Some(active_session(&owner)),
            user: Some(owner.clone()),
            fail: false,
        },
        Permissions(Ok(grants(&["organization.update"]))),
    );
    let principal = live
        .authenticate(&hash(), moment(11), None)
        .unwrap_or_else(|error| panic!("principal: {error}"));
    let actor = principal.audit_actor();
    assert_eq!(actor.actor_type(), "USER");
    assert_eq!(actor.user_id(), Some(owner.id()));
    assert_eq!(actor.system_provenance(), None);
}
