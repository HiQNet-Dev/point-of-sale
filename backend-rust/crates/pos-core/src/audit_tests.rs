// Los fixtures son constantes válidas; estos panics no pertenecen al código de producto.
#![allow(clippy::panic)]

use serde_json::{Map, Value, json};

use crate::{
    audit::{
        AuditAction, AuditActor, AuditEntityType, AuditError, AuditEventId, AuditEventRecord,
        AuditEventVersion, AuditMetadata, AuditOccurredAt, AuditReason,
        MAX_METADATA_SERIALIZED_BYTES, SystemProvenance,
    },
    organization_branch::EntityId,
};

fn id(value: &str) -> EntityId {
    EntityId::parse(value).unwrap_or_else(|error| panic!("fixture UUID: {error}"))
}

#[test]
fn metadata_accepts_nested_json_and_serializes_it_deterministically() {
    let first = AuditMetadata::new(json!({
        "z": [true, { "changedFields": ["name", "timezone"] }],
        "a": { "count": 2, "status": "ACTIVE" }
    }))
    .unwrap_or_else(|error| panic!("metadata: {error}"));
    let second = AuditMetadata::new(json!({
        "a": { "status": "ACTIVE", "count": 2 },
        "z": [true, { "changedFields": ["name", "timezone"] }]
    }))
    .unwrap_or_else(|error| panic!("metadata: {error}"));

    assert_eq!(first.canonical_json(), second.canonical_json());
    assert_eq!(
        first.canonical_json(),
        r#"{"a":{"count":2,"status":"ACTIVE"},"z":[true,{"changedFields":["name","timezone"]}]}"#
    );
}

#[test]
fn metadata_rejects_sensitive_keys_recursively_and_after_normalization() {
    for value in [
        json!({ "Password_Hash": "never" }),
        json!({ "context": { "Authorization Header": "never" } }),
        json!({ "items": [{ "clientSecret": "never" }] }),
        json!({ "event": { "jwt-token": "never" } }),
        json!({ "nested": [{ "private/key": "never" }] }),
    ] {
        assert_eq!(
            AuditMetadata::new(value),
            Err(AuditError::SensitiveMetadataKey)
        );
    }
}

#[test]
fn metadata_rejects_non_object_excessive_depth_and_oversize_without_truncation() {
    assert_eq!(
        AuditMetadata::new(json!(["not", "an", "object"])),
        Err(AuditError::MetadataRootMustBeObject)
    );

    let mut nested = json!("leaf");
    for _ in 0..32 {
        nested = json!({ "context": nested });
    }
    assert_eq!(AuditMetadata::new(nested), Err(AuditError::MetadataTooDeep));

    let too_large = "x".repeat(MAX_METADATA_SERIALIZED_BYTES);
    assert_eq!(
        AuditMetadata::new(json!({ "context": too_large })),
        Err(AuditError::MetadataTooLarge)
    );
}

#[test]
fn metadata_owns_its_json_tree_and_preserves_empty_as_absent_candidate() {
    let mut input = Map::new();
    input.insert("changedFields".to_owned(), json!(["name"]));
    let metadata = AuditMetadata::new(Value::Object(input.clone()))
        .unwrap_or_else(|error| panic!("metadata: {error}"));
    input.insert("password".to_owned(), json!("later input mutation"));

    assert_eq!(metadata.value(), &json!({ "changedFields": ["name"] }));
    assert!(AuditMetadata::empty().is_empty());
}

#[test]
fn event_requires_explicit_actor_identifiers_and_utc_timestamp() {
    let organization_id = id("00000000-0000-0000-0000-000000000001");
    let user_id = id("00000000-0000-0000-0000-000000000002");
    let event = AuditEventRecord {
        id: AuditEventId::new(id("00000000-0000-0000-0000-000000000003")),
        organization_id: organization_id.clone(),
        branch_id: None,
        actor: AuditActor::user(user_id.clone()),
        action: AuditAction::new("organization.updated")
            .unwrap_or_else(|error| panic!("action: {error}")),
        entity_type: AuditEntityType::new("organization")
            .unwrap_or_else(|error| panic!("type: {error}")),
        entity_id: Some(organization_id),
        reason: Some(
            AuditReason::new("corrección validada")
                .unwrap_or_else(|error| panic!("reason: {error}")),
        ),
        session_id: Some(id("00000000-0000-0000-0000-000000000004")),
        metadata_json: AuditMetadata::new(json!({ "changedFields": ["name"] }))
            .unwrap_or_else(|error| panic!("metadata: {error}")),
        occurred_at: AuditOccurredAt::new("2026-10-29T14:35:12.123Z")
            .unwrap_or_else(|error| panic!("timestamp: {error}")),
        event_version: AuditEventVersion::INITIAL,
    };

    assert_eq!(event.actor.actor_type(), "USER");
    assert_eq!(event.actor.user_id(), Some(&user_id));
    assert_eq!(
        AuditActor::system(SystemProvenance::SecureBootstrap).user_id(),
        None
    );
    assert_eq!(event.event_version.get(), 1);
    assert_eq!(
        AuditEventVersion::new(0),
        Err(AuditError::InvalidEventVersion)
    );
    assert!(AuditAction::new("Organization.updated").is_ok());
    assert_eq!(AuditAction::new(" \t\n"), Err(AuditError::InvalidAction));
    assert_eq!(
        AuditEntityType::new("\u{0000}"),
        Err(AuditError::InvalidEntityType)
    );
    assert_eq!(
        AuditOccurredAt::new("2026-10-29T14:35:12+00:00"),
        Err(AuditError::InvalidOccurredAt)
    );
    assert_eq!(
        AuditOccurredAt::new("2025-02-29T14:35:12Z"),
        Err(AuditError::InvalidOccurredAt)
    );
}

#[test]
fn reason_preserves_java_application_semantics_until_the_local_limit_is_approved() {
    assert!(AuditReason::new(" \t\n ").is_ok());
    assert!(AuditReason::new(&"😀".repeat(128)).is_ok());
    assert_eq!(AuditReason::new("\u{0000}"), Err(AuditError::InvalidReason));
}
