package com.hiqnet.pos.inventory.stock.api;

import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.PositiveOrZero;
import tools.jackson.databind.JsonNode;

public record AdjustInventoryStockRequest(
        @NotNull @PositiveOrZero Long expectedVersion,
        @NotNull JsonNode quantityDelta,
        @NotNull JsonNode reason) {}
