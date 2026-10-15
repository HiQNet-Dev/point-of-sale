package com.hiqnet.pos.inventory.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import java.time.Instant;
import java.util.UUID;

@Schema(description = "Cuenta de inventario de la organización con su unidad base y estado de bloqueo.")
public record InventoryItemResponse(
        UUID id,
        UUID organizationId,
        String code,
        String name,
        String description,
        InventoryBaseUnit baseUnit,
        boolean fractionalUnitAllowed,
        @Schema(description = "true cuando la unidad base está congelada porque el artículo tiene "
                        + "dependencias operativas o históricas")
                boolean unitLocked,
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
                item.isUnitLocked(),
                item.getStatus(),
                item.getVersion(),
                item.getCreatedAt(),
                item.getUpdatedAt());
    }
}
