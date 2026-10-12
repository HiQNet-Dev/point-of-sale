package com.hiqnet.pos.inventory.recipe.domain;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import java.math.BigDecimal;
import java.util.UUID;

public record RecipeComponentData(UUID inventoryItemId, String code, String name, InventoryBaseUnit baseUnit,
        boolean fractionalUnitAllowed, InventoryItemStatus status, BigDecimal quantityPerProduct) {}
