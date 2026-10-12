package com.hiqnet.pos.inventory.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

public record UpdateInventoryItemRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "0")
                @NotNull @Min(0) Long expectedVersion,
        @Size(max = 120) String name,
        @Size(max = 1000) String description,
        InventoryBaseUnit baseUnit,
        Boolean fractionalUnitAllowed,
        InventoryItemStatus status) {
}
