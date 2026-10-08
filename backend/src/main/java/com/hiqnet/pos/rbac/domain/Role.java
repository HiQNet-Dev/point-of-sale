package com.hiqnet.pos.rbac.domain;

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
import java.time.Instant;
import java.util.Locale;
import java.util.UUID;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/**
 * Organization-scoped role. {@code code} is a normalized slug, unique per
 * organization and immutable after creation; {@code name} is editable. Roles
 * are never physically deleted: deactivation is {@code status = INACTIVE},
 * which stops granting permissions while keeping assignments in the database.
 */
@Entity
@Table(
        name = "roles",
        uniqueConstraints = @UniqueConstraint(
                name = "roles_organization_code_unique",
                columnNames = {"organization_id", "code"}))
public class Role {

    private static final Pattern CODE_FORMAT = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_CODE = 64;
    private static final int MAX_NAME = 120;

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Column(nullable = false, length = MAX_CODE)
    private String code;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private RoleStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected Role() {
    }

    private Role(UUID organizationId, String code, String name) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.code = code;
        this.name = name.trim();
        this.status = RoleStatus.ACTIVE;
    }

    /**
     * Factory. {@code code} must already be a normalized slug; it is trimmed
     * defensively (never derived automatically from {@code name}).
     */
    public static Role create(UUID organizationId, String code, String name) {
        if (organizationId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "organizationId is required");
        }
        String normalizedCode = normalizeCode(code);
        if (name == null || name.isBlank()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "name is required");
        }
        if (name.trim().length() > MAX_NAME) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "name must not exceed " + MAX_NAME + " characters");
        }
        return new Role(organizationId, normalizedCode, name);
    }

    /** Normalization rule used by both the factory and the role service. */
    public static String normalizeCode(String code) {
        if (code == null || code.isBlank()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "code is required");
        }
        String normalized = code.trim().toLowerCase(Locale.ROOT);
        if (normalized.length() > MAX_CODE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "code must not exceed " + MAX_CODE + " characters");
        }
        if (!CODE_FORMAT.matcher(normalized).matches()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "code must match ^[a-z0-9]+(-[a-z0-9]+)*$");
        }
        return normalized;
    }

    public UUID getId() {
        return id;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public String getCode() {
        return code;
    }

    public String getName() {
        return name;
    }

    public RoleStatus getStatus() {
        return status;
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
        return status == RoleStatus.ACTIVE;
    }

    public void updateName(String newName) {
        if (newName == null || newName.isBlank()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "name is required");
        }
        if (newName.trim().length() > MAX_NAME) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "name must not exceed " + MAX_NAME + " characters");
        }
        this.name = newName.trim();
    }

    public void updateStatus(RoleStatus newStatus) {
        this.status = newStatus;
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