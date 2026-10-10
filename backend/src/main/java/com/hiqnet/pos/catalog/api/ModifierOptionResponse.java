package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionStatus;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

public record ModifierOptionResponse(
        UUID id,
        UUID organizationId,
        UUID modifierGroupId,
        String code,
        String name,
        BigDecimal priceDelta,
        int displayOrder,
        ModifierOptionStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static ModifierOptionResponse from(ModifierOption option) {
        return new ModifierOptionResponse(
                option.getId(),
                option.getOrganization().getId(),
                option.getModifierGroup().getId(),
                option.getCode(),
                option.getName(),
                option.getPriceDelta(),
                option.getDisplayOrder(),
                option.getStatus(),
                option.getVersion(),
                option.getCreatedAt(),
                option.getUpdatedAt());
    }
}
