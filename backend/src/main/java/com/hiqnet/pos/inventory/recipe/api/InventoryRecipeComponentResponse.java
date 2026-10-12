package com.hiqnet.pos.inventory.recipe.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.recipe.domain.RecipeComponentData;
import java.util.UUID;

public record InventoryRecipeComponentResponse(UUID inventoryItemId, String code, String name,
        InventoryBaseUnit baseUnit, boolean fractionalUnitAllowed, InventoryItemStatus status, String quantity) {
    public static InventoryRecipeComponentResponse from(RecipeComponentData component) {
        return new InventoryRecipeComponentResponse(component.inventoryItemId(), component.code(), component.name(),
                component.baseUnit(), component.fractionalUnitAllowed(), component.status(),
                com.hiqnet.pos.inventory.stock.application.InventoryQuantity.format(component.quantityPerProduct()));
    }
}
