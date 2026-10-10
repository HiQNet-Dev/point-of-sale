package com.hiqnet.pos.audit.domain;

/**
 * Actor of an audit event. USER events carry the acting identity
 * ({@code actor_user_id} required — the same organization is enforced by a
 * composite FK); SYSTEM events (future secure bootstrap, maintenance,
 * controlled internal processes) carry no user id. There are no synthetic
 * technical users.
 */
public enum AuditActorType {
    USER,
    SYSTEM
}