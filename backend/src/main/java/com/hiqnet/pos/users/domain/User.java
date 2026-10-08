package com.hiqnet.pos.users.domain;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import jakarta.persistence.Version;
import java.time.Duration;
import java.time.Instant;
import java.util.Locale;
import java.util.UUID;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/**
 * Minimal identity required by authentication: belongs to an organization,
 * email is normalized (trim + lowercase) and unique per organization, never
 * globally. Password is only ever stored as an externally-produced hash
 * (BCrypt via Spring Security); this module never sees plaintext.
 * Temporal lockout uses {@code lockedUntil}, not a permanent status.
 */
@Entity
@Table(
        name = "users",
        uniqueConstraints = @UniqueConstraint(
                name = "users_organization_email_unique",
                columnNames = {"organization_id", "email"}))
public class User {

    private static final int MAX_EMAIL = 254;

    private static final Pattern EMAIL_FORMAT =
            Pattern.compile("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$");

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Column(nullable = false, length = MAX_EMAIL)
    private String email;

    // 255 leaves room for future versioned/algorithm-prefixed hashes
    // (e.g. {bcrypt}..., Argon2) without forcing a schema migration. BCrypt
    // itself is ~60 chars.
    @Column(name = "password_hash", nullable = false, length = 255)
    private String passwordHash;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private UserStatus status;

    @Column(name = "failed_login_attempts", nullable = false)
    private int failedLoginAttempts;

    @Column(name = "locked_until")
    private Instant lockedUntil;

    @Column(name = "last_login_at")
    private Instant lastLoginAt;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected User() {
    }

    private User(UUID id, UUID organizationId, String email, String encodedPassword) {
        this.id = id;
        this.organizationId = organizationId;
        this.email = email;
        this.passwordHash = encodedPassword;
        this.status = UserStatus.ACTIVE;
        this.failedLoginAttempts = 0;
    }

    /**
     * Domain factory. {@code email} is normalized (trim + lowercase) before
     * being persisted. {@code encodedPassword} must already be a one-way hash
     * produced by a {@code PasswordEncoder}; the entity never handles
     * plaintext or performs hashing.
     */
    public static User create(UUID organizationId, String email, String encodedPassword) {
        if (organizationId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "organizationId is required");
        }
        String normalized = normalizeEmail(email);
        if (encodedPassword == null || encodedPassword.isBlank()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "passwordHash is required");
        }
        return new User(UUID.randomUUID(), organizationId, normalized, encodedPassword);
    }

    /**
     * Normalization rule used by both the factory and the login lookup, so a
     * login with any casing/whitespace variant finds the persisted user.
     */
    public static String normalizeEmail(String email) {
        if (email == null || email.isBlank()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "email is required");
        }
        String normalized = email.trim().toLowerCase(Locale.ROOT);
        if (normalized.length() > MAX_EMAIL) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "email must not exceed " + MAX_EMAIL + " characters");
        }
        if (!EMAIL_FORMAT.matcher(normalized).matches()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "email: must be a well-formed email address");
        }
        return normalized;
    }

    public UUID getId() {
        return id;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public String getEmail() {
        return email;
    }

    public String getPasswordHash() {
        return passwordHash;
    }

    public UserStatus getStatus() {
        return status;
    }

    public int getFailedLoginAttempts() {
        return failedLoginAttempts;
    }

    public Instant getLockedUntil() {
        return lockedUntil;
    }

    public Instant getLastLoginAt() {
        return lastLoginAt;
    }

    public long getVersion() {
        return version;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    public Instant getUpdatedAt() {
        return updatedAt;
    }

    public boolean isActive() {
        return status == UserStatus.ACTIVE;
    }

    public boolean isLocked(Instant now) {
        return lockedUntil != null && lockedUntil.isAfter(now);
    }

    /**
     * Called on a failed login. Increments the counter and, once the maximum
     * is reached, sets a temporary lock. The lock is time-based
     * ({@code lockedUntil}); there is no permanent LOCKED status.
     */
    public void recordFailedLogin(Instant now, int maxFailedAttempts, Duration lockoutDuration) {
        if (lockedUntil != null && !lockedUntil.isAfter(now)) {
            // Previous lock window already passed: count a fresh window.
            failedLoginAttempts = 0;
            lockedUntil = null;
        }
        this.failedLoginAttempts++;
        if (this.failedLoginAttempts >= maxFailedAttempts) {
            this.lockedUntil = now.plus(lockoutDuration);
        }
    }

    /**
     * Called on a successful login: clears failure state and records the
     * login time.
     */
    public void recordSuccessfulLogin(Instant now) {
        this.failedLoginAttempts = 0;
        this.lockedUntil = null;
        this.lastLoginAt = now;
    }

    public void deactivate() {
        this.status = UserStatus.INACTIVE;
    }

    /**
     * Reactivates a previously deactivated user. Previously revoked or consumed
     * refresh sessions do NOT come back to life: the user must authenticate
     * again, which is enforced entirely by the auth module without any
     * dependency back into it.
     */
    public void activate() {
        this.status = UserStatus.ACTIVE;
    }

    /**
     * Changes the identity email, applying the same normalization as
     * {@link #create} (trim + lowercase + format). The entity re-normalizes
     * defensively even when the caller already normalized the value.
     */
    public void changeEmail(String email) {
        this.email = normalizeEmail(email);
    }

    @PrePersist
    void onCreate() {
        Instant now = Instant.now();
        createdAt = now;
        updatedAt = now;
    }

    @PreUpdate
    void onUpdate() {
        updatedAt = Instant.now();
    }
}