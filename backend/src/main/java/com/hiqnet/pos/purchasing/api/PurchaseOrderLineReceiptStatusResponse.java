package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.purchasing.application.PurchaseOrderReceiptStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import java.util.UUID;

/**
 * Estado de recepción de una partida.
 */
@Schema(description = "Cantidades solicitada, recibida y pendiente de una partida.")
public record PurchaseOrderLineReceiptStatusResponse(
        @Schema(format = "uuid") UUID purchaseOrderLineId,
        @Schema(example = "1") int lineNumber,
        @Schema(format = "uuid") UUID inventoryItemId,
        @Schema(example = "GRAM") String unitSnapshot,
        @Schema(example = "10.000000") String quantityOrdered,
        @Schema(example = "4.000000") String quantityReceived,
        @Schema(example = "6.000000") String quantityPending) {

    /**
     * Convierte el estado de una partida en su representación HTTP.
     *
     * @param line estado de recepción de la partida
     * @return respuesta serializable de la partida
     */
    public static PurchaseOrderLineReceiptStatusResponse from(PurchaseOrderReceiptStatus.LineStatus line) {
        return new PurchaseOrderLineReceiptStatusResponse(
                line.line().getId(),
                line.line().getLineNumber(),
                line.line().getInventoryItemId(),
                line.line().getUnitSnapshot().name(),
                InventoryQuantity.format(line.line().getQuantity()),
                InventoryQuantity.format(line.quantityReceived()),
                InventoryQuantity.format(line.quantityPending()));
    }
}
