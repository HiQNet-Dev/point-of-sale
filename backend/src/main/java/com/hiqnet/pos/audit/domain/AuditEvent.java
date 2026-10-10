package com.hiqnet.pos.audit.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.Table;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.hibernate.annotations.Immutable;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

/**
 * ONE immutable, append-only record of an administrative/security mutation.
 *
 * <p>Immutability is a domain rule AND a database invariant: the entity exposes
 * no setters, no update methods and no way to drop, re-hash or replace its
 * metadata after creation (the JPA no-arg constructor is protected); the
 * {@code audit_events} table rejects every {@code UPDATE}/{@code DELETE} with a
 * BEFORE trigger — PostgreSQL is the final authority. {@code @Immutable} tells
 * Hibernate the same thing (no dirty checking, no accidental writing of an
 * {@code AuditEvent}); inserts are unaffected.</p>
 */
@Entity
@Immutable
@Table(name = "audit_events")
public class AuditEvent {

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Column(name = "branch_id")
    private UUID branchId;

    @Enumerated(EnumType.STRING)
    @Column(name = "actor_type", nullable = false, length = 16)
    private AuditActorType actorType;

    @Column(name = "actor_user_id")
    private UUID actorUserId;

    @Column(name = "token_jti", length = 64)
    private String tokenJti;

    @Column(nullable = false, length = 80)
    private String action;

    @Column(name = "entity_type", nullable = false, length = 40)
    private String entityType;

    @Column(name = "entity_id")
    private UUID entityId;

    @Column(length = 255)
    private String reason;

    @JdbcTypeCode(SqlTypes.JSON)
    @Column(columnDefinition = "jsonb")
    private Map<String, Object> metadata;

    @Column(name = "occurred_at", nullable = false, updatable = false)
    private Instant occurredAt;

    protected AuditEvent() {
    }

    private AuditEvent(
            UUID id,
            UUID organizationId,
            UUID branchId,
            AuditActorType actorType,
            UUID actorUserId,
            String tokenJti,
            String action,
            String entityType,
            UUID entityId,
            String reason,
            Map<String, Object> metadata,
            Instant occurredAt) {
        this.id = id;
        this.organizationId = organizationId;
        this.branchId = branchId;
        this.actorType = actorType;
        this.actorUserId = actorUserId;
        this.tokenJti = tokenJti;
        this.action = action;
        this.entityType = entityType;
        this.entityId = entityId;
        this.reason = reason;
        this.metadata = metadata;
        this.occurredAt = occurredAt;
    }

    /**
     * Factory. The {@link com.hiqnet.pos.shared.audit.AuditPort} implementation
     * resolves the actor (SecurityContext) and the {@code occurredAt}
     * (injectable Clock) centrally; the business service only provides the
     * event facts.
     */
    public static AuditEvent create(
            UUID organizationId,
            UUID branchId,
            AuditActorType actorType,
            UUID actorUserId,
            String tokenJti,
            String action,
            String entityType,
            UUID entityId,
            String reason,
            Map<String, Object> metadata,
            Instant occurredAt) {
        if (organizationId == null || actorType == null || action == null || entityType == null || occurredAt == null) {
            throw new IllegalArgumentException("organizationId, actorType, action, entityType and occurredAt are required");
        }
        return new AuditEvent(
                UUID.randomUUID(),
                organizationId,
                branchId,
                actorType,
                actorUserId,
                tokenJti,
                action,
                entityType,
                entityId,
                reason,
                metadata,
                occurredAt);
    }

    public UUID getId() {
        return id;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public UUID getBranchId() {
        return branchId;
    }

    public AuditActorType getActorType() {
        return actorType;
    }

    public UUID getActorUserId() {
        return actorUserId;
    }

    public String getTokenJti() {
        return tokenJti;
    }

    public String getAction() {
        return action;
    }

    public String getEntityType() {
        return entityType;
    }

    public UUID getEntityId() {
        return entityId;
    }

    public String getReason() {
        return reason;
    }

    public Map<String, Object> getMetadata() {
        // The Hibernate-hydrated map (and its nested maps/lists) is mutable:
        // a deep immutable view prevents any component from mutating an audit
        // event in memory through the getter. Values are scalars after
        // hydration, so this only wraps nested Maps/Collections.
        return metadata == null ? null : immutableView(metadata);
    }

    private static Map<String, Object> immutableView(Map<String, Object> source) {
        Map<String, Object> view = new LinkedHashMap<>();
        source.forEach((key, value) -> view.put(key, immutableViewValue(value)));
        return Collections.unmodifiableMap(view);
    }

    private static Object immutableViewValue(Object value) {
        if (value instanceof Map<?, ?> map) {
            Map<String, Object> view = new LinkedHashMap<>();
            map.forEach((key, item) -> view.put((String) key, immutableViewValue(item)));
            return Collections.unmodifiableMap(view);
        }
        if (value instanceof Collection<?> collection) {
            List<Object> view = new ArrayList<>(collection.size());
            for (Object item : collection) {
                view.add(immutableViewValue(item));
            }
            return Collections.unmodifiableList(view);
        }
        return value;
    }

    public Instant getOccurredAt() {
        return occurredAt;
    }
}