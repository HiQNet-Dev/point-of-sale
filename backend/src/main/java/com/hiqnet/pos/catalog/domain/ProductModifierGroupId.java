package com.hiqnet.pos.catalog.domain;

import java.io.Serializable;
import java.util.Objects;
import java.util.UUID;

/**
 * Composite identifier of {@link ProductModifierGroup}: a product references a
 * modifier group at most once. Matches the {@code (product_id, modifier_group_id)}
 * primary key of the join table.
 */
public class ProductModifierGroupId implements Serializable {

    private UUID productId;
    private UUID modifierGroupId;

    protected ProductModifierGroupId() {
    }

    public ProductModifierGroupId(UUID productId, UUID modifierGroupId) {
        this.productId = productId;
        this.modifierGroupId = modifierGroupId;
    }

    public UUID getProductId() {
        return productId;
    }

    public UUID getModifierGroupId() {
        return modifierGroupId;
    }

    @Override
    public boolean equals(Object other) {
        if (this == other) {
            return true;
        }
        if (!(other instanceof ProductModifierGroupId that)) {
            return false;
        }
        return Objects.equals(productId, that.productId) && Objects.equals(modifierGroupId, that.modifierGroupId);
    }

    @Override
    public int hashCode() {
        return Objects.hash(productId, modifierGroupId);
    }
}
