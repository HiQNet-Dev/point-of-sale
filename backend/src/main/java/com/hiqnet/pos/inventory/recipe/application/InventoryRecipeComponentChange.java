package com.hiqnet.pos.inventory.recipe.application;

import java.math.BigDecimal;
import java.util.UUID;

public record InventoryRecipeComponentChange(UUID inventoryItemId, BigDecimal quantityPerProduct) {}
