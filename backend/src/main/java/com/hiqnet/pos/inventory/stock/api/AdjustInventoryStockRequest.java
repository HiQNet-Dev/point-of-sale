package com.hiqnet.pos.inventory.stock.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.PositiveOrZero;
import tools.jackson.databind.JsonNode;

public record AdjustInventoryStockRequest(
        @NotNull @PositiveOrZero Long expectedVersion,
        @NotNull
                @Schema(
                        type = "string",
                        example = "-3.500000",
                        description = "Delta decimal con signo en formato string (hasta 6 decimales). Se documenta como string, no como number, para no perder precisión.")
                JsonNode quantityDelta,
        @NotNull
                @Schema(
                        type = "string",
                        description = "Motivo del ajuste; texto libre.")
                JsonNode reason) {}
