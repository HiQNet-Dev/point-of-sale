package com.hiqnet.pos.audit.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.Table;
import java.time.Instant;
import java.util.Map;
import java.util.UUID;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

/**
 * ONE immutable, append-only record of an administrative/security mutation.
 *
 * <p>Immutability is a domain rule AND a database invariant: the entity exposes
 * no setters, no update methods and no way to drop, re-hash or replace its
 * metadata after creation (the JPA no-arg constructor is protected); the
 * {@code audit_events} table rejects every {@code UPDATE}/{@code DELETE} with a
 * BEFORE trigger.</p>
 */
@Entity
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
        return metadata;
    }

    public Instant getOccurredAt() {
        return occurredAt;
    }
}