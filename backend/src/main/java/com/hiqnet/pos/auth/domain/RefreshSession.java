package com.hiqnet.pos.auth.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import java.time.Duration;
import java.time.Instant;
import java.util.UUID;

/**
 * One opaque refresh-token session. The raw token is never stored: only its
 * SHA-256 hash is persisted (high-entropy random token, so fast hashing is
 * appropriate). Sessions belong to a {@code family}: rotation creates a new
 * session in the same family and marks this one as rotated. The family has an
 * absolute expiry ({@code familyExpiresAt}) that rotation never extends, so a
 * session cannot renew itself forever.
 */
@Entity
@Table(
        name = "refresh_sessions",
        uniqueConstraints = @UniqueConstraint(
                name = "refresh_sessions_token_hash_unique",
                columnNames = {"token_hash"}))
public class RefreshSession {

    @Id
    private UUID id;

    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Column(name = "token_hash", nullable = false, length = 64)
    private String tokenHash;

    @Column(name = "family_id", nullable = false)
    private UUID familyId;

    @Column(name = "family_expires_at", nullable = false)
    private Instant familyExpiresAt;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "expires_at", nullable = false)
    private Instant expiresAt;

    @Column(name = "rotated_at")
    private Instant rotatedAt;

    @Column(name = "revoked_at")
    private Instant revokedAt;

    @Column(name = "replaced_by_session_id")
    private UUID replacedBySessionId;

    protected RefreshSession() {
    }

    private RefreshSession(
            UUID id, UUID userId, String tokenHash, UUID familyId, Instant now,
            Duration sessionTtl, Duration familyTtl) {
        this.id = id;
        this.userId = userId;
        this.tokenHash = tokenHash;
        this.familyId = familyId;
        this.createdAt = now;
        this.expiresAt = now.plus(sessionTtl);
        this.familyExpiresAt = now.plus(familyTtl);
    }

    /**
     * Creates the first session of a brand-new family.
     */
    public static RefreshSession create(
            UUID userId, String tokenHash, Instant now, Duration sessionTtl, Duration familyTtl) {
        return new RefreshSession(
                UUID.randomUUID(), userId, tokenHash, UUID.randomUUID(), now, sessionTtl, familyTtl);
    }

    /**
     * Rotates this session: marks it as rotated (it can never be used again)
     * and returns the new session of the same family. The family absolute
     * expiry is inherited, never extended.
     */
    public RefreshSession rotateTo(String newTokenHash, Instant now, Duration sessionTtl) {
        RefreshSession next = new RefreshSession(
                UUID.randomUUID(), this.userId, newTokenHash, this.familyId, now, sessionTtl, Duration.ZERO);
        next.familyExpiresAt = this.familyExpiresAt;
        this.rotatedAt = now;
        this.replacedBySessionId = next.id;
        return next;
    }

    /**
     * Revocation is idempotent: an already-revoked session stays revoked.
     */
    public void revoke(Instant now) {
        if (this.revokedAt == null) {
            this.revokedAt = now;
        }
    }

    public boolean isReusable(Instant now) {
        return rotatedAt == null
                && revokedAt == null
                && expiresAt.isAfter(now)
                && familyExpiresAt.isAfter(now);
    }

    /**
     * The instant this refresh token stops being usable: the earlier of the
     * per-session TTL ({@code expiresAt}) and the family absolute TTL
     * ({@code familyExpiresAt}). This is the value exposed to clients as
     * {@code refreshExpiresAt}, so the contract is unambiguous when the two
     * TTLs differ.
     */
    public Instant usableUntil() {
        return expiresAt.isBefore(familyExpiresAt) ? expiresAt : familyExpiresAt;
    }

    /**
     * True when this exact session was already rotated or revoked: presenting
     * such a token is treated as reuse and revokes the whole family.
     */
    public boolean wasRotatedOrRevoked() {
        return rotatedAt != null || revokedAt != null;
    }

    public UUID getId() {
        return id;
    }

    public UUID getUserId() {
        return userId;
    }

    public String getTokenHash() {
        return tokenHash;
    }

    public UUID getFamilyId() {
        return familyId;
    }

    public Instant getFamilyExpiresAt() {
        return familyExpiresAt;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    public Instant getExpiresAt() {
        return expiresAt;
    }

    public Instant getRotatedAt() {
        return rotatedAt;
    }

    public Instant getRevokedAt() {
        return revokedAt;
    }

    public UUID getReplacedBySessionId() {
        return replacedBySessionId;
    }

    @PrePersist
    void onCreate() {
        if (createdAt == null) {
            createdAt = Instant.now();
        }
    }
}