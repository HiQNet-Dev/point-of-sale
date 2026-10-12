package com.hiqnet.pos.inventory.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import java.time.Instant;
import java.util.UUID;

public record InventoryItemResponse(
        UUID id,
        UUID organizationId,
        String code,
        String name,
        String description,
        InventoryBaseUnit baseUnit,
        boolean fractionalUnitAllowed,
        InventoryItemStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static InventoryItemResponse from(InventoryItem item) {
        return new InventoryItemResponse(
                item.getId(),
                item.getOrganization().getId(),
                item.getCode(),
                item.getName(),
                item.getDescription(),
                item.getBaseUnit(),
                item.isFractionalUnitAllowed(),
                item.getStatus(),
                item.getVersion(),
                item.getCreatedAt(),
                item.getUpdatedAt());
    }
}
