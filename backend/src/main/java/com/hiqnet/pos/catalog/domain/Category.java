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
 * Organization-owned product category. The catalog belongs to the organization
 * (catalog = organization, operation = branch). {@code code} is a normalized
 * slug, unique per organization and immutable. No hard delete: a category is
 * retired via status INACTIVE, which never cascades to the status of its
 * products.
 */
@Entity
@Table(
        name = "categories",
        uniqueConstraints =
                @UniqueConstraint(
                        name = "categories_organization_code_unique",
                        columnNames = {"organization_id", "code"}))
public class Category {

    private static final Pattern SLUG = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_NAME = 120;
    private static final int MAX_CODE = 64;

    @Id
    private UUID id;

    // EAGER (default for ManyToOne): every category response carries its
    // organization id; DTO mapping happens outside the service transaction.
    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false)
    private Organization organization;

    @Column(nullable = false, length = MAX_CODE)
    private String code;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private CategoryStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected Category() {
    }

    private Category(UUID id, Organization organization, String code, String name) {
        this.id = id;
        this.organization = organization;
        this.code = code;
        this.name = name;
        this.status = CategoryStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Domain factory. Caller must have validated that the organization exists
     * and that the code is free within it. {@code name} is trimmed here;
     * {@code code} is expected already normalized (trimmed, lowercased).
     */
    public static Category create(Organization organization, String code, String name) {
        validateName(name);
        validateCode(code);
        return new Category(UUID.randomUUID(), organization, code, name.trim());
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

    public CategoryStatus getStatus() {
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
        return status == CategoryStatus.ACTIVE;
    }

    public void updateName(String newName) {
        validateName(newName);
        this.name = newName.trim();
    }

    public void updateStatus(CategoryStatus newStatus) {
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
