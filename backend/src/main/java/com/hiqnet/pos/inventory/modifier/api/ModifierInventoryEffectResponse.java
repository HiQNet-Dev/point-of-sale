package com.hiqnet.pos.inventory.modifier.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.modifier.domain.ModifierInventoryEffectData;
import java.util.UUID;

public record ModifierInventoryEffectResponse(UUID inventoryItemId, String code, String name,
        InventoryBaseUnit baseUnit, boolean fractionalUnitAllowed, InventoryItemStatus status, String quantityDelta) {
    public static ModifierInventoryEffectResponse from(ModifierInventoryEffectData effect) {
        return new ModifierInventoryEffectResponse(effect.inventoryItemId(), effect.code(), effect.name(),
                effect.baseUnit(), effect.fractionalUnitAllowed(), effect.status(),
                com.hiqnet.pos.inventory.stock.application.InventoryQuantity.format(effect.quantityDelta()));
    }
}
