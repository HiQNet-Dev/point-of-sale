package com.hiqnet.pos.catalog.domain;

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
 * Organization-owned reusable modifier group (e.g. "Milk", "Size"). The modifier
 * catalog belongs to the organization (modifier = organization, operation =
 * branch). {@code code} is a normalized slug, unique per organization and
 * immutable.
 *
 * <p>Selection semantics are DERIVED, never stored: {@code required} =
 * {@code minSelections > 0}, {@code single} = {@code maxSelections == 1},
 * {@code multi} = {@code maxSelections > 1}. {@code minSelections} must be
 * {@code >= 0}, {@code maxSelections} must be {@code >= 1} and
 * {@code minSelections <= maxSelections}. No hard delete: a group is retired via
 * status INACTIVE, which never cascades to its options or product assignments.
 */
@Entity
@Table(
        name = "modifier_groups",
        uniqueConstraints =
                @UniqueConstraint(
                        name = "modifier_groups_organization_code_unique",
                        columnNames = {"organization_id", "code"}))
public class ModifierGroup {

    private static final Pattern SLUG = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_NAME = 120;
    private static final int MAX_CODE = 64;

    @Id
    private UUID id;

    // EAGER (default for ManyToOne): every response carries its organization id;
    // DTO mapping happens outside the service transaction.
    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false)
    private Organization organization;

    @Column(nullable = false, length = MAX_CODE)
    private String code;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Column(name = "min_selections", nullable = false)
    private int minSelections;

    @Column(name = "max_selections", nullable = false)
    private int maxSelections;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private ModifierGroupStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected ModifierGroup() {
    }

    private ModifierGroup(
            UUID id,
            Organization organization,
            String code,
            String name,
            int minSelections,
            int maxSelections) {
        this.id = id;
        this.organization = organization;
        this.code = code;
        this.name = name;
        this.minSelections = minSelections;
        this.maxSelections = maxSelections;
        this.status = ModifierGroupStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Domain factory. Caller must have validated that the organization exists
     * and that the code is free within it. {@code name} is trimmed here;
     * {@code code} is expected already normalized (trimmed, lowercased).
     */
    public static ModifierGroup create(
            Organization organization, String code, String name, int minSelections, int maxSelections) {
        validateName(name);
        validateCode(code);
        validateSelections(minSelections, maxSelections);
        return new ModifierGroup(UUID.randomUUID(), organization, code, name.trim(), minSelections, maxSelections);
    }

    public UUID getId() {
        return id;
    }

    public Organization getOrganization() {
        return organization;
    }

    public String getCode() {
        return code;
    }

    public String getName() {
        return name;
    }

    public int getMinSelections() {
        return minSelections;
    }

    public int getMaxSelections() {
        return maxSelections;
    }

    public ModifierGroupStatus getStatus() {
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
        return status == ModifierGroupStatus.ACTIVE;
    }

    /** Required: at least one option must be selected (derived, never stored). */
    public boolean isRequired() {
        return minSelections > 0;
    }

    /** Single choice: exactly one option (derived, never stored). */
    public boolean isSingleChoice() {
        return maxSelections == 1;
    }

    /** Multiple choice: more than one option (derived, never stored). */
    public boolean isMultipleChoice() {
        return maxSelections > 1;
    }

    public void updateName(String newName) {
        validateName(newName);
        this.name = newName.trim();
    }

    public void updateSelections(int newMin, int newMax) {
        validateSelections(newMin, newMax);
        this.minSelections = newMin;
        this.maxSelections = newMax;
    }

    public void updateStatus(ModifierGroupStatus newStatus) {
        if (newStatus != null) {
            this.status = newStatus;
        }
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

    private static void validateSelections(int min, int max) {
        if (min < 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "minSelections must be >= 0");
        }
        if (max < 1) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "maxSelections must be >= 1");
        }
        if (min > max) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "minSelections must not exceed maxSelections");
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
