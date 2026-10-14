package com.hiqnet.pos.inventory.modifier.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;
import tools.jackson.databind.JsonNode;

public record ModifierInventoryEffectRequest(
        @NotNull UUID inventoryItemId,
        @NotNull
                @Schema(
                        type = "string",
                        example = "-1.000000",
                        description = "Delta decimal con signo en formato string (hasta 6 decimales). Se documenta como string, no como number, para no perder precisión.")
                JsonNode quantityDelta) {}
