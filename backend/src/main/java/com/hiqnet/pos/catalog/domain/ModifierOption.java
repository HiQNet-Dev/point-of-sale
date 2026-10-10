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
 * Organization-owned option of a {@link ModifierGroup} (e.g. "Whole", "Skim").
 * {@code code} is a normalized slug, unique WITHIN the group and immutable.
 *
 * <p>{@code priceDelta} is a SIGNED delta added on top of the product base price
 * (negative = discount, zero = no change, positive = surcharge); the currency is
 * inherited from the organization and never stored per option.
 * {@code displayOrder} is the ordering hint within the group ({@code >= 0}). No
 * hard delete: an option is retired via status INACTIVE.
 */
@Entity
@Table(
        name = "modifier_options",
        uniqueConstraints =
                @UniqueConstraint(
                        name = "modifier_options_group_code_unique",
                        columnNames = {"modifier_group_id", "code"}))
public class ModifierOption {

    private static final Pattern SLUG = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_NAME = 120;
    private static final int MAX_CODE = 64;
    private static final int PRICE_SCALE = 4;
    private static final int PRICE_INTEGER_DIGITS = 15; // NUMERIC(19,4)

    @Id
    private UUID id;

    // EAGER: every response carries its organization id and owning group id;
    // DTO mapping happens outside the service transaction.
    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false)
    private Organization organization;

    @ManyToOne(optional = false)
    @JoinColumn(name = "modifier_group_id", nullable = false)
    private ModifierGroup modifierGroup;

    @Column(nullable = false, length = MAX_CODE)
    private String code;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Column(name = "price_delta", nullable = false, precision = 19, scale = 4)
    private BigDecimal priceDelta;

    @Column(name = "display_order", nullable = false)
    private int displayOrder;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private ModifierOptionStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected ModifierOption() {
    }

    private ModifierOption(
            UUID id,
            Organization organization,
            ModifierGroup modifierGroup,
            String code,
            String name,
            BigDecimal priceDelta,
            int displayOrder) {
        this.id = id;
        this.organization = organization;
        this.modifierGroup = modifierGroup;
        this.code = code;
        this.name = name;
        this.priceDelta = priceDelta;
        this.displayOrder = displayOrder;
        this.status = ModifierOptionStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Domain factory. Caller must have validated that the group exists in the
     * SAME organization and that the code is free within the group. {@code name}
     * is trimmed here; {@code code} is expected already normalized.
     */
    public static ModifierOption create(
            Organization organization,
            ModifierGroup modifierGroup,
            String code,
            String name,
            BigDecimal priceDelta,
            int displayOrder) {
        validateName(name);
        validateCode(code);
        validatePriceDelta(priceDelta);
        validateDisplayOrder(displayOrder);
        return new ModifierOption(
                UUID.randomUUID(), organization, modifierGroup, code, name.trim(), priceDelta, displayOrder);
    }

    public UUID getId() {
        return id;
    }

    public Organization getOrganization() {
        return organization;
    }

    public ModifierGroup getModifierGroup() {
        return modifierGroup;
    }

    public String getCode() {
        return code;
    }

    public String getName() {
        return name;
    }

    public BigDecimal getPriceDelta() {
        return priceDelta;
    }

    public int getDisplayOrder() {
        return displayOrder;
    }

    public ModifierOptionStatus getStatus() {
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
        return status == ModifierOptionStatus.ACTIVE;
    }

    public void updateName(String newName) {
        validateName(newName);
        this.name = newName.trim();
    }

    public void updatePriceDelta(BigDecimal newPriceDelta) {
        validatePriceDelta(newPriceDelta);
        this.priceDelta = newPriceDelta;
    }

    public void updateDisplayOrder(int newDisplayOrder) {
        validateDisplayOrder(newDisplayOrder);
        this.displayOrder = newDisplayOrder;
    }

    public void updateStatus(ModifierOptionStatus newStatus) {
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

    private static void validatePriceDelta(BigDecimal value) {
        if (value == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "priceDelta is required");
        }
        if (value.scale() > PRICE_SCALE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "priceDelta must have at most " + PRICE_SCALE + " decimal places");
        }
        if (value.precision() - value.scale() > PRICE_INTEGER_DIGITS) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "priceDelta is out of range");
        }
    }

    private static void validateDisplayOrder(int value) {
        if (value < 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "displayOrder must be >= 0");
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
