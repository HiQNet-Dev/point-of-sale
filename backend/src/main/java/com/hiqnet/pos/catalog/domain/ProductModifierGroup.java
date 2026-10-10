package com.hiqnet.pos.catalog.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.IdClass;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import java.time.Instant;
import java.util.UUID;

/**
 * Ordered configuration link: a product referencing a modifier group at a
 * position ({@code displayOrder}). The product references a group at most once
 * (composite primary key). This is a plain join entity: tenant integrity is
 * enforced at the database level by composite FKs and concurrency by the owning
 * {@link Product} version column.
 */
@Entity
@Table(name = "product_modifier_groups")
@IdClass(ProductModifierGroupId.class)
public class ProductModifierGroup {

    @Id
    @Column(name = "product_id", nullable = false)
    private UUID productId;

    @Id
    @Column(name = "modifier_group_id", nullable = false)
    private UUID modifierGroupId;

    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Column(name = "display_order", nullable = false)
    private int displayOrder;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    protected ProductModifierGroup() {
    }

    private ProductModifierGroup(
            UUID organizationId, UUID productId, UUID modifierGroupId, int displayOrder) {
        this.organizationId = organizationId;
        this.productId = productId;
        this.modifierGroupId = modifierGroupId;
        this.displayOrder = displayOrder;
    }

    public static ProductModifierGroup of(
            UUID organizationId, UUID productId, UUID modifierGroupId, int displayOrder) {
        if (displayOrder < 0) {
            throw new IllegalArgumentException("displayOrder must be >= 0");
        }
        return new ProductModifierGroup(organizationId, productId, modifierGroupId, displayOrder);
    }

    public UUID getProductId() {
        return productId;
    }

    public UUID getModifierGroupId() {
        return modifierGroupId;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public int getDisplayOrder() {
        return displayOrder;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    @PrePersist
    void onCreate() {
        createdAt = Instant.now();
    }
}
