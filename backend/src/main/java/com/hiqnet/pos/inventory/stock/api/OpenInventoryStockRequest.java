package com.hiqnet.pos.inventory.stock.api;

import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.PositiveOrZero;
import tools.jackson.databind.JsonNode;

public record OpenInventoryStockRequest(
        @NotNull @PositiveOrZero Long expectedVersion,
        @NotNull JsonNode quantity,
        JsonNode reason) {}
