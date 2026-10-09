package com.hiqnet.pos.shared.audit;

/**
 * Transversal application contract used by every business module to record an
 * administrative/security mutation. Modules depend ONLY on this interface —
 * never on {@code audit} infrastructure classes — so the compile direction
 * stays {@code module -> shared.audit} and {@code audit -> shared.audit}.
 *
 * <p>The implementation (in the {@code audit} module) resolves the actor from
 * the security context, stores only the JWT {@code jti}, stamps the time with
 * an injectable {@code Clock} and REQUIRES an existing transaction
 * ({@code Propagation.MANDATORY}): the audit insert is part of the SAME
 * transaction as the business mutation — one atomic commit, and a failed audit
 * rolls the mutation back.</p>
 */
public interface AuditPort {

    /**
     * Records the event inside the CURRENT transaction. Must be called from
     * within an active transaction (the implementation enforces it); if the
     * surrounding business transaction rolls back, the event rolls back too.
     */
    void record(AuditRecord record);
}