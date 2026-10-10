package com.hiqnet.pos.audit.application;

import java.time.Instant;
import java.util.UUID;

/**
 * Optional read filters of the audit log. Time semantics: {@code from} is
 * INCLUSIVE, {@code to} is EXCLUSIVE ({@code [from, to)}); {@code from > to}
 * is rejected with 400. {@code branchId} and {@code actorUserId} are validated
 * against the organization (cross-organization ids → 404, never a silent empty
 * result).
 */
public record AuditEventFilter(
        UUID branchId,
        UUID actorUserId,
        String action,
        String entityType,
        UUID entityId,
        Instant from,
        Instant to) {}