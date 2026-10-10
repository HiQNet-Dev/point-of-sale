package com.hiqnet.pos.shared.audit;

import java.util.UUID;

/**
 * The event data a business service submits for auditing. It carries ONLY the
 * facts the service knows and is responsible for; the actor identity, the
 * access-token jti and the occurredAt timestamp are resolved centrally by the
 * {@link AuditPort} implementation (services never touch Spring Security for
 * auditing).
 *
 * <p>Action/entity conventions (documented in {@code docs/04-security/audit.md}):
 * {@code action} follows {@code entity.action} (e.g. {@code user.deactivated}),
 * {@code entityType} follows the stable machine names ({@code organization},
 * {@code branch}, {@code user}, {@code role}, {@code branch_membership}).</p>
 */
public record AuditRecord(
        UUID organizationId,
        UUID branchId,
        String action,
        String entityType,
        UUID entityId,
        String reason,
        AuditMetadata metadata) {

    public AuditRecord {
        if (organizationId == null) {
            throw new IllegalArgumentException("organizationId is required");
        }
        if (action == null || action.trim().isEmpty()) {
            throw new IllegalArgumentException("action is required");
        }
        if (entityType == null || entityType.trim().isEmpty()) {
            throw new IllegalArgumentException("entityType is required");
        }
    }

    /** Organization-scoped event with no branch and no reason. */
    public static AuditRecord of(
            UUID organizationId, String action, String entityType, UUID entityId, AuditMetadata metadata) {
        return new AuditRecord(organizationId, null, action, entityType, entityId, null, metadata);
    }

    /** Organization-scoped event with a reason (future actions) and no branch. */
    public static AuditRecord of(
            UUID organizationId,
            String action,
            String entityType,
            UUID entityId,
            String reason,
            AuditMetadata metadata) {
        return new AuditRecord(organizationId, null, action, entityType, entityId, reason, metadata);
    }

    /** Branch-scoped event (branch belongs to the organization — DB enforces it). */
    public static AuditRecord ofBranch(
            UUID organizationId,
            UUID branchId,
            String action,
            String entityType,
            UUID entityId,
            AuditMetadata metadata) {
        return new AuditRecord(organizationId, branchId, action, entityType, entityId, null, metadata);
    }
}