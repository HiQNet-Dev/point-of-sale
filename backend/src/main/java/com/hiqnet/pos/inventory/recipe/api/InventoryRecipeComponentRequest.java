package com.hiqnet.pos.inventory.recipe.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;
import tools.jackson.databind.JsonNode;

public record InventoryRecipeComponentRequest(
        @NotNull UUID inventoryItemId,
        @NotNull
                @Schema(
                        type = "string",
                        example = "0.250000",
                        description = "Cantidad decimal en formato string (hasta 6 decimales). Se documenta como string, no como number, para no perder precisión.")
                JsonNode quantity) {}
