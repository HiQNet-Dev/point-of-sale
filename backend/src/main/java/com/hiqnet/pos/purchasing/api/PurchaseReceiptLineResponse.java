package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.purchasing.domain.PurchaseReceiptLine;
import io.swagger.v3.oas.annotations.media.Schema;
import java.util.UUID;

/**
 * Representación pública de una línea de recepción.
 */
@Schema(description = "Cantidad recibida de una partida en una recepción.")
public record PurchaseReceiptLineResponse(
        @Schema(format = "uuid") UUID id,
        @Schema(format = "uuid") UUID purchaseOrderLineId,
        @Schema(format = "uuid") UUID inventoryItemId,
        @Schema(example = "4.000000", description = "Cantidad recibida como string decimal de escala 6.")
                String quantityReceived) {

    /**
     * Convierte la línea persistida en su representación HTTP.
     *
     * @param line línea de recepción persistida
     * @return respuesta serializable de la línea
     */
    public static PurchaseReceiptLineResponse from(PurchaseReceiptLine line) {
        return new PurchaseReceiptLineResponse(
                line.getId(),
                line.getPurchaseOrderLineId(),
                line.getInventoryItemId(),
                InventoryQuantity.format(line.getQuantityReceived()));
    }
}
