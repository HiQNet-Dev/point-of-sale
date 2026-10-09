package com.hiqnet.pos.shared.audit;

/**
 * Auditing is an internal invariant: a record that cannot be safely persisted
 * (sensitive metadata key, metadata over the size limit, broken actor, …)
 * fails the surrounding business transaction — the mutation rolls back instead
 * of committing without its audit history (append-only, strong atomicity).
 */
public class AuditException extends RuntimeException {

    public AuditException(String message) {
        super(message);
    }
}