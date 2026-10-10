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
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/**
 * Organization-owned product with a base unit price. The catalog belongs to the
 * organization (catalog = organization, operation = branch).
 *
 * <p>{@code sku} is the immutable business identifier (trimmed, uppercase
 * canonical, unique per organization) and is distinct from the UUID.
 * {@code barcode} is optional and unique per organization when present; it is
 * kept as text to preserve leading zeros. {@code unitPrice} is the organization
 * base price; the currency is inherited from the organization and never stored
 * per product. No hard delete: a product is retired via status INACTIVE.
 */
@Entity
@Table(
        name = "products",
        uniqueConstraints =
                @UniqueConstraint(
                        name = "products_organization_sku_unique",
                        columnNames = {"organization_id", "sku"}))
public class Product {

    private static final Pattern SKU = Pattern.compile("^[A-Z0-9._-]{1,64}$");
    private static final int MAX_SKU = 64;
    private static final int MAX_BARCODE = 64;
    private static final int MAX_NAME = 120;
    private static final int MAX_DESCRIPTION = 1000;
    private static final int PRICE_SCALE = 4;
    private static final int PRICE_INTEGER_DIGITS = 15; // NUMERIC(19,4)

    @Id
    private UUID id;

    // EAGER (default for ManyToOne): every product response carries its
    // organization id; DTO mapping happens outside the service transaction.
    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false)
    private Organization organization;

    // Nullable: a product may exist without a category.
    @ManyToOne
    @JoinColumn(name = "category_id")
    private Category category;

    @Column(nullable = false, length = MAX_SKU)
    private String sku;

    @Column(length = MAX_BARCODE)
    private String barcode;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Column(length = MAX_DESCRIPTION)
    private String description;

    @Column(name = "unit_price", nullable = false, precision = 19, scale = 4)
    private BigDecimal unitPrice;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private ProductStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected Product() {
    }

    private Product(UUID id, Organization organization, String sku, String name, BigDecimal unitPrice) {
        this.id = id;
        this.organization = organization;
        this.sku = sku;
        this.name = name;
        this.unitPrice = unitPrice;
        this.status = ProductStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Domain factory. Caller must have validated that the organization exists
     * and that the SKU is free within it. {@code sku} is expected already
     * normalized (trimmed, uppercased); {@code name} is trimmed here.
     */
    public static Product create(Organization organization, String sku, String name, BigDecimal unitPrice) {
        validateSku(sku);
        validateName(name);
        validateUnitPrice(unitPrice);
        return new Product(UUID.randomUUID(), organization, sku, name.trim(), unitPrice);
    }

    public UUID getId() {
        return id;
    }

    public Organization getOrganization() {
        return organization;
    }

    public Category getCategory() {
        return category;
    }

    public String getSku() {
        return sku;
    }

    public String getBarcode() {
        return barcode;
    }

    public String getName() {
        return name;
    }

    public String getDescription() {
        return description;
    }

    public BigDecimal getUnitPrice() {
        return unitPrice;
    }

    public ProductStatus getStatus() {
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
        return status == ProductStatus.ACTIVE;
    }

    public void updateName(String newName) {
        validateName(newName);
        this.name = newName.trim();
    }

    /**
     * Blank is normalized to {@code null} so "no description" has one
     * representation.
     */
    public void updateDescription(String newDescription) {
        validateDescription(newDescription);
        this.description = normalizeOptional(newDescription);
    }

    public void updateBarcode(String newBarcode) {
        validateBarcode(newBarcode);
        this.barcode = normalizeOptional(newBarcode);
    }

    public void updateUnitPrice(BigDecimal newUnitPrice) {
        validateUnitPrice(newUnitPrice);
        this.unitPrice = newUnitPrice;
    }

    public void assignCategory(Category newCategory) {
        this.category = newCategory;
    }

    public void removeCategory() {
        this.category = null;
    }

    public void updateStatus(ProductStatus newStatus) {
        if (newStatus != null) {
            this.status = newStatus;
        }
    }

    private static String normalizeOptional(String value) {
        if (value == null || value.trim().isEmpty()) {
            return null;
        }
        return value.trim();
    }

    private static void validateSku(String value) {
        if (value == null || !SKU.matcher(value).matches()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "sku must match ^[A-Z0-9._-]{1,64}$");
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

    private static void validateDescription(String value) {
        if (value == null) {
            return;
        }
        if (value.trim().length() > MAX_DESCRIPTION) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "description must not exceed " + MAX_DESCRIPTION + " characters");
        }
    }

    private static void validateBarcode(String value) {
        if (value == null) {
            return;
        }
        if (value.trim().length() > MAX_BARCODE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "barcode must not exceed " + MAX_BARCODE + " characters");
        }
    }

    private static void validateUnitPrice(BigDecimal value) {
        if (value == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "unitPrice is required");
        }
        if (value.signum() < 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "unitPrice must be >= 0");
        }
        if (value.scale() > PRICE_SCALE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "unitPrice must have at most " + PRICE_SCALE + " decimal places");
        }
        if (value.precision() - value.scale() > PRICE_INTEGER_DIGITS) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "unitPrice is out of range");
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
