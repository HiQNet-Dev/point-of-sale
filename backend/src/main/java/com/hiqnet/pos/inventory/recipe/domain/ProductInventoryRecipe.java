package com.hiqnet.pos.inventory.recipe.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import jakarta.persistence.Version;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "product_inventory_recipes")
public class ProductInventoryRecipe {
    @Id private UUID id;
    @Column(name = "organization_id", nullable = false, updatable = false) private UUID organizationId;
    @Column(name = "product_id", nullable = false, updatable = false) private UUID productId;
    @Enumerated(EnumType.STRING) @Column(nullable = false) private InventoryRecipeStatus status;
    @Version @Column(nullable = false) private long version;
    @Column(name = "created_at", nullable = false, updatable = false) private Instant createdAt;
    @Column(name = "updated_at", nullable = false) private Instant updatedAt;

    protected ProductInventoryRecipe() {}
    public UUID getId() { return id; }
    public UUID getOrganizationId() { return organizationId; }
    public UUID getProductId() { return productId; }
    public InventoryRecipeStatus getStatus() { return status; }
    public long getVersion() { return version; }
    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }

    public void replace(InventoryRecipeStatus newStatus) {
        status = newStatus;
        Instant now = Instant.now();
        updatedAt = updatedAt != null && !now.isAfter(updatedAt) ? updatedAt.plusNanos(1_000) : now;
    }

    @PreUpdate
    void onUpdate() { updatedAt = Instant.now(); }
}
