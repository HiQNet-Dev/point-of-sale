package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;
import tools.jackson.databind.JsonNode;

/**
 * Cantidad recibida de una partida en una recepción.
 */
@Schema(description = "Cantidad recibida de una partida, como string decimal exacto.")
public record PostPurchaseReceiptLineRequest(
        @Schema(description = "Partida de la orden que se recibe.", example = "7d5c1a2b-3e4f-4a5b-8c9d-0e1f2a3b4c5d")
                @NotNull UUID purchaseOrderLineId,
        @NotNull
                @Schema(
                        type = "string",
                        example = "4.000000",
                        description = "Cantidad recibida en la unidad base de la partida, como string decimal (NUMERIC(19,6)).")
                JsonNode quantity) {
}
