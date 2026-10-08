package com.hiqnet.pos.branch.domain;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.JoinColumn;
import jakarta.persistence.ManyToOne;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import jakarta.persistence.Version;
import java.time.Instant;
import java.util.UUID;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/**
 * Operational boundary of the POS: owns cash registers and inventory.
 * Always scoped to exactly one organization; {@code code} is unique only
 * within that organization. {@code timezone} may be null to inherit the
 * organization timezone.
 */
@Entity
@Table(
        name = "branches",
        uniqueConstraints =
                @UniqueConstraint(name = "branches_organization_code_unique", columnNames = {"organization_id", "code"}))
public class Branch {

    private static final Pattern SLUG = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_NAME = 120;
    private static final int MAX_CODE = 64;
    private static final int MAX_TIMEZONE = 64;

    @Id
    private UUID id;

    // EAGER (default for ManyToOne): every branch response carries its
    // organization id; DTO mapping happens outside the service transaction.
    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false)
    private Organization organization;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Column(nullable = false, length = MAX_CODE)
    private String code;

    @Column(length = MAX_TIMEZONE)
    private String timezone;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private BranchStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected Branch() {
    }

    private Branch(UUID id, Organization organization, String name, String code, String timezone) {
        this.id = id;
        this.organization = organization;
        this.name = name;
        this.code = code;
        this.timezone = timezone;
        this.status = BranchStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Domain factory. Caller must have validated that the organization exists
     * and that the code is free within it.
     */
    public static Branch create(Organization organization, String name, String code, String timezone) {
        validateName(name);
        validateCode(code);
        validateTimezoneOrNull(timezone);
        return new Branch(UUID.randomUUID(), organization, name, code, timezone);
    }

    public UUID getId() {
        return id;
    }

    public Organization getOrganization() {
        return organization;
    }

    public String getName() {
        return name;
    }

    public String getCode() {
        return code;
    }

    public String getTimezone() {
        return timezone;
    }

    public BranchStatus getStatus() {
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

    /**
     * Timezone resolution rule: the branch inherits the organization timezone
     * unless it declares its own.
     */
    public String effectiveTimezone() {
        return timezone != null ? timezone : organization.getTimezone();
    }

    public void updateName(String newName) {
        validateName(newName);
        this.name = newName;
    }

    public void updateTimezone(String newTimezone) {
        validateTimezoneOrNull(newTimezone);
        this.timezone = newTimezone;
    }

    public void updateStatus(BranchStatus newStatus) {
        this.status = newStatus;
    }

    private static void validateName(String value) {
        if (value == null || value.trim().isEmpty()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "name is required");
        }
        if (value.trim().length() > MAX_NAME) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "name must not exceed " + MAX_NAME + " characters");
        }
    }

    private static void validateCode(String value) {
        if (value == null || !SLUG.matcher(value).matches()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_CODE,
                    "code must match ^[a-z0-9]+(-[a-z0-9]+)*$");
        }
        if (value.length() > MAX_CODE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_CODE,
                    "code must not exceed " + MAX_CODE + " characters");
        }
    }

    private static void validateTimezoneOrNull(String value) {
        if (value == null) {
            return;
        }
        if (value.isBlank() || value.length() > MAX_TIMEZONE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_TIMEZONE, "timezone must be a valid IANA timezone");
        }
        try {
            java.time.ZoneId.of(value);
        } catch (java.time.DateTimeException e) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_TIMEZONE,
                    "timezone must be a valid IANA timezone, e.g. America/Mexico_City");
        }
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