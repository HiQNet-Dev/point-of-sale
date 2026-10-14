package com.hiqnet.pos.inventory.stock.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.PositiveOrZero;
import tools.jackson.databind.JsonNode;

public record OpenInventoryStockRequest(
        @NotNull @PositiveOrZero Long expectedVersion,
        @NotNull
                @Schema(
                        type = "string",
                        example = "100.000000",
                        description = "Cantidad decimal en formato string (hasta 6 decimales). Se documenta como string, no como number, para no perder precisión.")
                JsonNode quantity,
        @Schema(
                type = "string",
                description = "Motivo opcional del movimiento; texto libre.")
        JsonNode reason) {}
