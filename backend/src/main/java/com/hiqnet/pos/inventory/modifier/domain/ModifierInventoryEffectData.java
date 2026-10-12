package com.hiqnet.pos.inventory.modifier.domain;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import java.math.BigDecimal;
import java.util.UUID;

public record ModifierInventoryEffectData(UUID modifierOptionId, UUID inventoryItemId, String code, String name,
        InventoryBaseUnit baseUnit, boolean fractionalUnitAllowed, InventoryItemStatus status, BigDecimal quantityDelta) {}
