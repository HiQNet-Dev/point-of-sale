package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;
import tools.jackson.databind.JsonNode;

/**
 * Datos para agregar una partida a un borrador.
 */
@Schema(description = "Datos de una nueva partida. El artículo debe ser de la misma organización y estar activo.")
public record CreatePurchaseOrderLineRequest(
        @Schema(description = "Versión del agregado leída por el cliente.", example = "0")
                @NotNull @Min(0) Long expectedVersion,
        @Schema(description = "Artículo de inventario de la organización.", example = "b6d2a1f3-6a4e-4a2e-9a3f-2f1c8d7b6a50")
                @NotNull UUID inventoryItemId,
        @NotNull
                @Schema(
                        type = "string",
                        example = "25000.000000",
                        description = "Cantidad solicitada en la unidad base del artículo, como string decimal (hasta 6 decimales, estrictamente positiva).")
                JsonNode quantity,
        @Schema(
                        type = "string",
                        nullable = true,
                        example = "0.0180",
                        description = "Costo unitario pactado opcional, como string decimal (hasta 4 decimales, no negativo). Omitir o null si no se conoce.")
                JsonNode unitCost) {
}
