package com.hiqnet.pos.audit.api;

import com.hiqnet.pos.audit.domain.AuditActorType;
import com.hiqnet.pos.audit.domain.AuditEvent;
import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * Administrative view of one audit event. Never exposes JPA internals and never
 * contains token material (only the {@code jti} correlation id of the access
 * JWT, which is not a secret).
 */
public record AuditResponse(
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

    public static AuditResponse from(AuditEvent event) {
        return new AuditResponse(
                event.getId(),
                event.getOrganizationId(),
                event.getBranchId(),
                event.getActorType(),
                event.getActorUserId(),
                event.getTokenJti(),
                event.getAction(),
                event.getEntityType(),
                event.getEntityId(),
                event.getReason(),
                event.getMetadata(),
                event.getOccurredAt());
    }
}