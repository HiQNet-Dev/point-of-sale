package com.hiqnet.pos.sales.domain;

import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

/** Server-built immutable snapshot of a selected modifier on a sale item. */
@Entity
@Table(name = "sale_item_modifiers")
public class SaleItemModifier {
    @Id private UUID id;
    @Column(name = "organization_id", nullable = false) private UUID organizationId;
    @Column(name = "sale_item_id", nullable = false) private UUID saleItemId;
    @Column(name = "modifier_group_id", nullable = false) private UUID modifierGroupId;
    @Column(name = "modifier_option_id", nullable = false) private UUID modifierOptionId;
    @Column(name = "group_code_snapshot", nullable = false) private String groupCodeSnapshot;
    @Column(name = "group_name_snapshot", nullable = false) private String groupNameSnapshot;
    @Column(name = "option_code_snapshot", nullable = false) private String optionCodeSnapshot;
    @Column(name = "option_name_snapshot", nullable = false) private String optionNameSnapshot;
    @Column(name = "price_delta_snapshot", nullable = false, precision = 19, scale = 4) private BigDecimal priceDeltaSnapshot;
    @Column(name = "display_order", nullable = false) private int displayOrder;
    @Column(name = "created_at", nullable = false, updatable = false) private Instant createdAt;

    protected SaleItemModifier() {}
    private SaleItemModifier(UUID organizationId, UUID saleItemId, ModifierGroup group, ModifierOption option, int displayOrder) {
        this.id = UUID.randomUUID(); this.organizationId = organizationId; this.saleItemId = saleItemId;
        this.modifierGroupId = group.getId(); this.modifierOptionId = option.getId();
        this.groupCodeSnapshot = group.getCode(); this.groupNameSnapshot = group.getName();
        this.optionCodeSnapshot = option.getCode(); this.optionNameSnapshot = option.getName();
        this.priceDeltaSnapshot = option.getPriceDelta().setScale(4); this.displayOrder = displayOrder;
    }
    public static SaleItemModifier snapshot(UUID organizationId, UUID saleItemId, ModifierGroup group, ModifierOption option, int displayOrder) {
        return new SaleItemModifier(organizationId, saleItemId, group, option, displayOrder);
    }
    public UUID getId() { return id; }
    public UUID getSaleItemId() { return saleItemId; }
    public UUID getModifierGroupId() { return modifierGroupId; }
    public UUID getModifierOptionId() { return modifierOptionId; }
    public String getGroupCodeSnapshot() { return groupCodeSnapshot; }
    public String getGroupNameSnapshot() { return groupNameSnapshot; }
    public String getOptionCodeSnapshot() { return optionCodeSnapshot; }
    public String getOptionNameSnapshot() { return optionNameSnapshot; }
    public BigDecimal getPriceDeltaSnapshot() { return priceDeltaSnapshot; }
    public int getDisplayOrder() { return displayOrder; }
    public Instant getCreatedAt() { return createdAt; }
    @PrePersist void onCreate() { createdAt = Instant.now(); }
}
