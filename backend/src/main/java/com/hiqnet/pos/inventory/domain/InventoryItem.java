package com.hiqnet.pos.inventory.domain;

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

/** Organization-owned definition of a material or consumable tracked by Inventory. */
@Entity
@Table(
        name = "inventory_items",
        uniqueConstraints = @UniqueConstraint(
                name = "inventory_items_organization_code_unique",
                columnNames = {"organization_id", "code"}))
public class InventoryItem {

    private static final Pattern CODE = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_CODE = 64;
    private static final int MAX_NAME = 120;
    private static final int MAX_DESCRIPTION = 1000;

    @Id
    private UUID id;

    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false, updatable = false)
    private Organization organization;

    @Column(nullable = false, length = MAX_CODE, updatable = false)
    private String code;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Column(length = MAX_DESCRIPTION)
    private String description;

    @Enumerated(EnumType.STRING)
    @Column(name = "base_unit", nullable = false, length = 16)
    private InventoryBaseUnit baseUnit;

    @Column(name = "fractional_unit_allowed", nullable = false)
    private boolean fractionalUnitAllowed;

    /**
     * Unidad base congelada una vez que el artículo tiene dependencias
     * operativas o históricas.
     *
     * <p>Es <b>monótona</b>: pasa a {@code true} una sola vez y nunca regresa a
     * {@code false}. Para garantizarlo el mapeo es {@code updatable = false}: una
     * actualización de estado de la entidad (flush de Hibernate) jamás escribe
     * esta columna, por lo que una instancia administrada obsoleta (cargada con
     * {@code false} antes de marcarla) no puede revertir el valor persistido. Solo
     * la sentencia dedicada de {@code InventoryItemUnitGuard} la activa.</p>
     */
    @Column(name = "unit_locked", nullable = false, updatable = false)
    private boolean unitLocked;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private InventoryItemStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected InventoryItem() {
    }

    private InventoryItem(
            Organization organization,
            String code,
            String name,
            String description,
            InventoryBaseUnit baseUnit,
            boolean fractionalUnitAllowed) {
        this.id = UUID.randomUUID();
        this.organization = organization;
        this.code = code;
        this.name = normalizeName(name);
        this.description = normalizeDescription(description);
        this.baseUnit = requireBaseUnit(baseUnit);
        this.fractionalUnitAllowed = fractionalUnitAllowed;
        this.status = InventoryItemStatus.ACTIVE;
    }

    public static InventoryItem create(
            Organization organization,
            String code,
            String name,
            String description,
            InventoryBaseUnit baseUnit,
            boolean fractionalUnitAllowed) {
        validateCode(code);
        return new InventoryItem(organization, code, name, description, baseUnit, fractionalUnitAllowed);
    }

    public UUID getId() { return id; }
    public Organization getOrganization() { return organization; }
    public String getCode() { return code; }
    public String getName() { return name; }
    public String getDescription() { return description; }
    public InventoryBaseUnit getBaseUnit() { return baseUnit; }
    public boolean isFractionalUnitAllowed() { return fractionalUnitAllowed; }
    public boolean isUnitLocked() { return unitLocked; }
    public InventoryItemStatus getStatus() { return status; }
    public long getVersion() { return version; }
    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }

    public void updateName(String value) {
        this.name = normalizeName(value);
    }

    public void updateDescription(String value) {
        this.description = normalizeDescription(value);
    }

    public void updateBaseUnit(InventoryBaseUnit value) {
        this.baseUnit = requireBaseUnit(value);
    }

    public void updateFractionalUnitAllowed(boolean value) {
        this.fractionalUnitAllowed = value;
    }

    public void updateStatus(InventoryItemStatus value) {
        if (value == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "status is required");
        }
        this.status = value;
    }

    private static String normalizeName(String value) {
        if (value == null || value.trim().isEmpty()) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "name is required");
        }
        String normalized = value.trim();
        if (normalized.length() > MAX_NAME) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR,
                    "name must not exceed " + MAX_NAME + " characters");
        }
        return normalized;
    }

    private static String normalizeDescription(String value) {
        if (value == null || value.trim().isEmpty()) {
            return null;
        }
        String normalized = value.trim();
        if (normalized.length() > MAX_DESCRIPTION) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR,
                    "description must not exceed " + MAX_DESCRIPTION + " characters");
        }
        return normalized;
    }

    private static InventoryBaseUnit requireBaseUnit(InventoryBaseUnit value) {
        if (value == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "baseUnit is required");
        }
        return value;
    }

    private static void validateCode(String value) {
        if (value == null || value.length() > MAX_CODE || !CODE.matcher(value).matches()) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_CODE,
                    "code must match ^[a-z0-9]+(-[a-z0-9]+)*$ and not exceed " + MAX_CODE + " characters");
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
