package com.hiqnet.pos.inventory.recipe.api;

import jakarta.validation.constraints.NotNull;
import java.util.UUID;
import tools.jackson.databind.JsonNode;

public record InventoryRecipeComponentRequest(@NotNull UUID inventoryItemId, @NotNull JsonNode quantity) {}
