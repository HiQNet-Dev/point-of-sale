// Fixtures deterministas para reglas puras; no representan secretos de producto.
#![allow(clippy::panic)]

use std::{collections::BTreeSet, future::Future};

use serde_json::json;

use crate::{
    audit::AuditAttributionError,
    identity::{
        AuthorizationError, CredentialAlgorithm, CredentialKind, CredentialParameters,
        CredentialState, IdentityError, LocalCredential, LocalIdentityRepository,
        LocalPrincipalService, LocalSession, LocalSessionState, LockoutPolicy, NewCredential,
        NormalizedEmail, PERMISSION_CATALOG, PermissionAuthority, PermissionCode, PosUser,
        PosUserPatch, PosUserStatus, PrincipalScope, SecretHash, SessionHash, UtcSeconds,
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
fn permission_catalog_matches_the_java_catalog_and_parses_identically() {
    assert_eq!(PERMISSION_CATALOG.len(), 59);
    let mut seen = BTreeSet::new();
    for code in PERMISSION_CATALOG {
        let parsed =
            PermissionCode::new(code).unwrap_or_else(|error| panic!("código {code}: {error}"));
        assert_eq!(parsed.as_str(), code);
        assert!(seen.insert(code), "código duplicado: {code}");
    }
    assert!(PermissionCode::new("cash-register.read").is_ok());
    assert!(PermissionCode::new("purchasing.order.write").is_ok());
    assert_eq!(
        PermissionCode::new("Cash.register"),
        Err(IdentityError::InvalidPermission)
    );
    assert_eq!(
        PermissionCode::new("organization..read"),
        Err(IdentityError::InvalidPermission)
    );
    assert_eq!(
        PermissionCode::new("organization.read."),
        Err(IdentityError::InvalidPermission)
    );
    assert_eq!(
        PermissionCode::new("organization read"),
        Err(IdentityError::InvalidPermission)
    );
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
    assert_eq!(
        LocalSession::restore(
            session.id().clone(),
            session.user_id().clone(),
            session.organization_id().clone(),
            hash(),
            moment(100),
            moment(100),
            None,
            1
        ),
        Err(IdentityError::StorageUnavailable)
    );
}

/// Fuente única de identidad y permisos para pruebas: nunca es un adaptador real.
#[derive(Clone, Default)]
struct Source {
    session: Option<LocalSession>,
    user: Option<PosUser>,
    grants: Option<Result<BTreeSet<PermissionCode>, IdentityError>>,
    fail: bool,
}
impl Source {
    fn granting(session: LocalSession, user: PosUser, codes: &[&str]) -> Self {
        Self {
            session: Some(session),
            user: Some(user),
            grants: Some(Ok(codes.iter().map(|code| permission(code)).collect())),
            fail: false,
        }
    }
}
impl LocalIdentityRepository for Source {
    fn find_session_by_hash(
        &mut self,
        provided: &SessionHash,
    ) -> impl Future<Output = Result<Option<LocalSession>, IdentityError>> + Send {
        let result = if self.fail {
            Err(IdentityError::StorageUnavailable)
        } else {
            Ok(self
                .session
                .as_ref()
                .filter(|session| session.session_hash() == provided)
                .cloned())
        };
        async move { result }
    }
    fn find_user(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> impl Future<Output = Result<Option<PosUser>, IdentityError>> + Send {
        let result = Ok(self
            .user
            .as_ref()
            .filter(|user| user.organization_id() == organization_id && user.id() == user_id)
            .cloned());
        async move { result }
    }
}
impl PermissionAuthority for Source {
    fn resolve_permissions(
        &mut self,
        _: &EntityId,
        _: &EntityId,
        _: Option<&EntityId>,
    ) -> impl Future<Output = Result<BTreeSet<PermissionCode>, IdentityError>> + Send {
        let result = self.grants.clone().unwrap_or_else(|| Ok(BTreeSet::new()));
        async move { result }
    }
    fn resolve_permissions_for_recovery(
        &mut self,
        _: &EntityId,
        _: &EntityId,
        _: &EntityId,
    ) -> impl Future<Output = Result<BTreeSet<PermissionCode>, IdentityError>> + Send {
        let result = self.grants.clone().unwrap_or_else(|| Ok(BTreeSet::new()));
        async move { result }
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

async fn build_principal(
    source: Source,
    branch: Option<&EntityId>,
) -> Result<crate::identity::VerifiedPrincipal, AuditAttributionError> {
    LocalPrincipalService::new(source)
        .authenticate(&hash(), moment(11), branch)
        .await
}

#[tokio::test]
async fn principal_retains_branch_scope_and_denies_other_scopes() {
    let owner = user();
    let branch_a = id("00000000-0000-0000-0000-000000000201");
    let branch_b = id("00000000-0000-0000-0000-000000000202");
    let principal = build_principal(
        Source::granting(active_session(&owner), owner.clone(), &["branch.update"]),
        Some(&branch_a),
    )
    .await
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

    let org_principal = build_principal(
        Source::granting(
            active_session(&owner),
            owner.clone(),
            &["organization.update"],
        ),
        None,
    )
    .await
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

#[tokio::test]
async fn recovery_scope_is_explicit_and_never_reusable_for_normal_operations() {
    let owner = user();
    let branch_a = id("00000000-0000-0000-0000-000000000201");
    let recovery = LocalPrincipalService::new(Source::granting(
        active_session(&owner),
        owner.clone(),
        &["cash.close"],
    ))
    .authenticate_recovery(&hash(), moment(11), &branch_a)
    .await
    .unwrap_or_else(|error| panic!("principal: {error}"));
    let recovery_scope =
        PrincipalScope::recovery(owner.organization_id().clone(), branch_a.clone());
    let normal_scope = PrincipalScope::new(owner.organization_id().clone(), Some(branch_a.clone()));
    assert_eq!(
        recovery.scope().mode(),
        crate::identity::ScopeMode::Recovery
    );
    assert_eq!(
        recovery.authorize(&permission("cash.close"), &recovery_scope),
        Ok(())
    );
    assert_eq!(
        recovery.authorize(&permission("cash.close"), &normal_scope),
        Err(AuthorizationError::ScopeMismatch)
    );

    let normal = LocalPrincipalService::new(Source::granting(
        active_session(&owner),
        owner.clone(),
        &["cash.close"],
    ))
    .authenticate(&hash(), moment(11), Some(&branch_a))
    .await
    .unwrap_or_else(|error| panic!("principal: {error}"));
    assert_eq!(
        normal.authorize(&permission("cash.close"), &recovery_scope),
        Err(AuthorizationError::ScopeMismatch)
    );
}

#[tokio::test]
async fn principal_fails_closed_for_unknown_inactive_expired_and_authority_errors() {
    let owner = user();
    assert_eq!(
        build_principal(Source::default(), None).await,
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
    assert_eq!(
        build_principal(
            Source {
                session: Some(active_session(&inactive)),
                user: Some(inactive),
                ..Source::default()
            },
            None
        )
        .await,
        Err(AuditAttributionError::Unauthenticated)
    );

    let expired = LocalPrincipalService::new(Source {
        session: Some(active_session(&owner)),
        user: Some(owner.clone()),
        ..Source::default()
    })
    .authenticate(&hash(), moment(1_001), None)
    .await;
    assert_eq!(expired, Err(AuditAttributionError::Unauthenticated));

    assert_eq!(
        build_principal(
            Source {
                fail: true,
                ..Source::default()
            },
            None
        )
        .await,
        Err(AuditAttributionError::UnattributablePrincipal)
    );

    assert_eq!(
        build_principal(
            Source {
                session: Some(active_session(&owner)),
                user: Some(owner.clone()),
                grants: Some(Err(IdentityError::InvalidPermission)),
                fail: false,
            },
            None
        )
        .await,
        Err(AuditAttributionError::ScopeDenied)
    );
}

#[tokio::test]
async fn revoked_session_cannot_build_a_principal_or_an_actor() {
    let owner = user();
    let mut session = active_session(&owner);
    session
        .revoke(1, moment(20))
        .unwrap_or_else(|error| panic!("revoke: {error}"));
    assert_eq!(
        build_principal(
            Source {
                session: Some(session),
                user: Some(owner.clone()),
                ..Source::default()
            },
            None
        )
        .await,
        Err(AuditAttributionError::Unauthenticated)
    );

    let live = build_principal(
        Source::granting(
            active_session(&owner),
            owner.clone(),
            &["organization.update"],
        ),
        None,
    )
    .await
    .unwrap_or_else(|error| panic!("principal: {error}"));
    let actor = live.audit_actor();
    assert_eq!(actor.actor_type(), "USER");
    assert_eq!(actor.user_id(), Some(owner.id()));
    assert_eq!(actor.system_provenance(), None);
}
