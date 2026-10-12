package com.hiqnet.pos.inventory.application;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;

public record UpdateInventoryItemCommand(
        long expectedVersion,
        String name,
        String description,
        InventoryBaseUnit baseUnit,
        Boolean fractionalUnitAllowed,
        InventoryItemStatus status) {
}
