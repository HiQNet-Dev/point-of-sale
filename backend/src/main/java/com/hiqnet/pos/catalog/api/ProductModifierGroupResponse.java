package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;
import java.util.UUID;

/** A modifier group attached to a product, with its position in the list. */
public record ProductModifierGroupResponse(
        UUID modifierGroupId,
        String code,
        String name,
        int minSelections,
        int maxSelections,
        ModifierGroupStatus status,
        int displayOrder) {

    public static ProductModifierGroupResponse from(ModifierGroup group, int displayOrder) {
        return new ProductModifierGroupResponse(
                group.getId(),
                group.getCode(),
                group.getName(),
                group.getMinSelections(),
                group.getMaxSelections(),
                group.getStatus(),
                displayOrder);
    }
}
