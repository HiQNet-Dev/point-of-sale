package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.application.PurchaseOrderAmounts;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderLine;
import io.swagger.v3.oas.annotations.media.Schema;
import java.util.UUID;

/**
 * Representación pública de una partida de orden de compra.
 */
@Schema(description = "Partida de una orden de compra con snapshots históricos del artículo.")
public record PurchaseOrderLineResponse(
        @Schema(format = "uuid", example = "1c9f9a9e-6c2a-4f7a-9d1e-0c2b7a3f4d51") UUID id,
        @Schema(format = "uuid", example = "b6d2a1f3-6a4e-4a2e-9a3f-2f1c8d7b6a50") UUID inventoryItemId,
        @Schema(example = "1") int lineNumber,
        @Schema(example = "harina-trigo") String itemCodeSnapshot,
        @Schema(example = "Harina de trigo") String itemNameSnapshot,
        @Schema(example = "GRAM", description = "Unidad base del artículo capturada al crear la partida.") String unitSnapshot,
        @Schema(example = "25000.000000", description = "Cantidad solicitada como string decimal (escala 6).") String quantity,
        @Schema(example = "0.0180", description = "Costo unitario pactado como string decimal (escala 4), o null si no se informó.") String unitCost) {

    /**
     * Convierte la partida persistida en su representación HTTP.
     *
     * @param line partida persistida
     * @return respuesta serializable de la partida
     */
    public static PurchaseOrderLineResponse from(PurchaseOrderLine line) {
        return new PurchaseOrderLineResponse(
                line.getId(),
                line.getInventoryItemId(),
                line.getLineNumber(),
                line.getItemCodeSnapshot(),
                line.getItemNameSnapshot(),
                line.getUnitSnapshot().name(),
                PurchaseOrderAmounts.formatQuantity(line.getQuantity()),
                PurchaseOrderAmounts.formatUnitCost(line.getUnitCost()));
    }
}
