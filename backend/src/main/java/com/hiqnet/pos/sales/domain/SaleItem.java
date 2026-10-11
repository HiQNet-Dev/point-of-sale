package com.hiqnet.pos.sales.domain;

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
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;

/** Immutable catalog facts quoted on a sale line, with logical removal only. */
@Entity
@Table(name = "sale_items")
public class SaleItem {

    @Id
    private UUID id;
    @Column(name = "organization_id", nullable = false) private UUID organizationId;
    @Column(name = "sale_id", nullable = false) private UUID saleId;
    @Column(name = "product_id", nullable = false) private UUID productId;
    @Column(name = "product_sku_snapshot", nullable = false) private String productSkuSnapshot;
    @Column(name = "product_name_snapshot", nullable = false) private String productNameSnapshot;
    @Column(name = "base_unit_price", nullable = false, precision = 19, scale = 4) private BigDecimal baseUnitPrice;
    @Column(name = "modifier_unit_delta", nullable = false, precision = 19, scale = 4) private BigDecimal modifierUnitDelta;
    @Column(name = "effective_unit_price", nullable = false, precision = 19, scale = 4) private BigDecimal effectiveUnitPrice;
    @Column(nullable = false) private int quantity;
    @Column(name = "line_total", nullable = false, precision = 19, scale = 4) private BigDecimal lineTotal;
    @Enumerated(EnumType.STRING) @Column(nullable = false, length = 16) private SaleItemStatus status;
    @Column(nullable = false) private int position;
    @Column(name = "created_at", nullable = false, updatable = false) private Instant createdAt;
    @Column(name = "updated_at", nullable = false) private Instant updatedAt;
    @Column(name = "removed_at") private Instant removedAt;
    @Column(name = "removed_by_user_id") private UUID removedByUserId;

    protected SaleItem() {}

    private SaleItem(UUID organizationId, UUID saleId, UUID productId, String sku, String name,
            BigDecimal basePrice, BigDecimal modifierDelta, BigDecimal effectivePrice, int quantity, int position) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.saleId = saleId;
        this.productId = productId;
        this.position = position;
        this.status = SaleItemStatus.ACTIVE;
        replaceQuote(sku, name, basePrice, modifierDelta, effectivePrice, quantity);
    }

    public static SaleItem create(UUID organizationId, UUID saleId, UUID productId, String sku, String name,
            BigDecimal basePrice, BigDecimal modifierDelta, BigDecimal effectivePrice, int quantity, int position) {
        return new SaleItem(organizationId, saleId, productId, sku, name, basePrice, modifierDelta, effectivePrice, quantity, position);
    }

    public void updateQuote(String sku, String name, BigDecimal basePrice, BigDecimal modifierDelta, BigDecimal effectivePrice, int quantity) {
        requireActive();
        replaceQuote(sku, name, basePrice, modifierDelta, effectivePrice, quantity);
    }

    private void replaceQuote(String sku, String name, BigDecimal basePrice, BigDecimal modifierDelta, BigDecimal effectivePrice, int quantity) {
        if (sku == null || sku.isBlank() || name == null || name.isBlank() || quantity < 1 || quantity > 1_000_000
                || basePrice == null || modifierDelta == null || effectivePrice == null || effectivePrice.signum() < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "Invalid sale item quote");
        }
        this.productSkuSnapshot = sku;
        this.productNameSnapshot = name;
        this.baseUnitPrice = basePrice.setScale(4);
        this.modifierUnitDelta = modifierDelta.setScale(4);
        this.effectiveUnitPrice = effectivePrice.setScale(4);
        this.quantity = quantity;
        try {
            this.lineTotal = effectivePrice.multiply(BigDecimal.valueOf(quantity)).setScale(4);
        } catch (ArithmeticException ex) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "lineTotal is out of range");
        }
        if (lineTotal.precision() - lineTotal.scale() > 15) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "lineTotal is out of range");
        }
    }

    public void remove(UUID actorUserId, Instant when) {
        if (status == SaleItemStatus.REMOVED) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.SALE_ITEM_ALREADY_REMOVED, "The sale item is already removed");
        }
        status = SaleItemStatus.REMOVED;
        removedByUserId = actorUserId;
        removedAt = when;
    }

    public void requireActive() {
        if (status != SaleItemStatus.ACTIVE) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.SALE_ITEM_ALREADY_REMOVED, "The sale item is removed");
        }
    }

    public UUID getId() { return id; }
    public UUID getOrganizationId() { return organizationId; }
    public UUID getSaleId() { return saleId; }
    public UUID getProductId() { return productId; }
    public String getProductSkuSnapshot() { return productSkuSnapshot; }
    public String getProductNameSnapshot() { return productNameSnapshot; }
    public BigDecimal getBaseUnitPrice() { return baseUnitPrice; }
    public BigDecimal getModifierUnitDelta() { return modifierUnitDelta; }
    public BigDecimal getEffectiveUnitPrice() { return effectiveUnitPrice; }
    public int getQuantity() { return quantity; }
    public BigDecimal getLineTotal() { return lineTotal; }
    public SaleItemStatus getStatus() { return status; }
    public int getPosition() { return position; }
    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }
    public Instant getRemovedAt() { return removedAt; }
    public UUID getRemovedByUserId() { return removedByUserId; }

    @PrePersist void onCreate() { Instant now = Instant.now(); createdAt = now; updatedAt = now; }
    @PreUpdate void onUpdate() { updatedAt = Instant.now(); }
}
