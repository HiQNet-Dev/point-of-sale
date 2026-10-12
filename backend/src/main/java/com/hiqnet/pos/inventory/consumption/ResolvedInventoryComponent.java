package com.hiqnet.pos.inventory.consumption;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import java.math.BigDecimal;
import java.util.UUID;

public record ResolvedInventoryComponent(UUID inventoryItemId, String code, String name,
        InventoryBaseUnit baseUnit, boolean fractionalUnitAllowed, BigDecimal quantity) {}
