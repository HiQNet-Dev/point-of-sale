package com.hiqnet.pos.sales.api;

import com.hiqnet.pos.sales.domain.SaleItemModifier;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

public record SaleItemModifierResponse(UUID id, UUID modifierGroupId, UUID modifierOptionId,
        String groupCodeSnapshot, String groupNameSnapshot, String optionCodeSnapshot, String optionNameSnapshot,
        BigDecimal priceDeltaSnapshot, int displayOrder, Instant createdAt) {
    static SaleItemModifierResponse from(SaleItemModifier modifier) {
        return new SaleItemModifierResponse(modifier.getId(), modifier.getModifierGroupId(), modifier.getModifierOptionId(),
                modifier.getGroupCodeSnapshot(), modifier.getGroupNameSnapshot(), modifier.getOptionCodeSnapshot(),
                modifier.getOptionNameSnapshot(), modifier.getPriceDeltaSnapshot(), modifier.getDisplayOrder(), modifier.getCreatedAt());
    }
}
