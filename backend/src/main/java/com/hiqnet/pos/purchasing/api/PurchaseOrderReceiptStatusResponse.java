package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.application.PurchaseOrderReceiptStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import java.util.List;
import java.util.UUID;

/**
 * Respuesta con el estado de recepción de una orden: cantidades solicitadas,
 * recibidas y pendientes por partida.
 */
@Schema(description = "Cantidades solicitadas, recibidas y pendientes de una orden de compra.")
public record PurchaseOrderReceiptStatusResponse(
        @Schema(format = "uuid") UUID purchaseOrderId,
        @Schema(example = "PARTIALLY_RECEIVED") String status,
        @Schema(example = "2") long version,
        List<PurchaseOrderLineReceiptStatusResponse> lines) {

    /**
     * Convierte el estado de recepción en su representación HTTP.
     *
     * @param status estado de recepción por partida
     * @return respuesta serializable del estado
     */
    public static PurchaseOrderReceiptStatusResponse from(PurchaseOrderReceiptStatus status) {
        return new PurchaseOrderReceiptStatusResponse(
                status.order().getId(),
                status.order().getStatus().name(),
                status.order().getVersion(),
                status.lines().stream().map(PurchaseOrderLineReceiptStatusResponse::from).toList());
    }
}
