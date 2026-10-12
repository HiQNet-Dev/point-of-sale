package com.hiqnet.pos.inventory.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

public record CreateInventoryItemRequest(
        @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @NotNull @Size(max = 120) String name,
        @Size(max = 1000) String description,
        @NotNull InventoryBaseUnit baseUnit,
        Boolean fractionalUnitAllowed) {
}
