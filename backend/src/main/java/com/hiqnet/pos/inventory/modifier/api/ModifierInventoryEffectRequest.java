package com.hiqnet.pos.inventory.modifier.api;

import jakarta.validation.constraints.NotNull;
import java.util.UUID;
import tools.jackson.databind.JsonNode;

public record ModifierInventoryEffectRequest(@NotNull UUID inventoryItemId, @NotNull JsonNode quantityDelta) {}
