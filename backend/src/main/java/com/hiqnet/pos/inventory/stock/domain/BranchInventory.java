package com.hiqnet.pos.inventory.stock.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import jakarta.persistence.Version;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "branch_inventory")
public class BranchInventory {

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false, updatable = false)
    private UUID organizationId;

    @Column(name = "branch_id", nullable = false, updatable = false)
    private UUID branchId;

    @Column(name = "inventory_item_id", nullable = false, updatable = false)
    private UUID inventoryItemId;

    @Column(name = "quantity_on_hand", nullable = false, precision = 19, scale = 6)
    private BigDecimal quantityOnHand;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected BranchInventory() {}

    public UUID getId() { return id; }

    public UUID getOrganizationId() { return organizationId; }

    public UUID getBranchId() { return branchId; }

    public UUID getInventoryItemId() { return inventoryItemId; }

    public BigDecimal getQuantityOnHand() { return quantityOnHand; }

    public long getVersion() { return version; }

    public void setQuantityOnHand(BigDecimal value) { quantityOnHand = value; }

    @PrePersist
    void onCreate() {
        Instant now = Instant.now();
        createdAt = now;
        updatedAt = now;
    }

    @PreUpdate
    void onUpdate() { updatedAt = Instant.now(); }
}
