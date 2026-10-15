package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.application.PurchaseOrderDetail;
import io.swagger.v3.oas.annotations.media.Schema;
import java.util.List;

/**
 * Respuesta de detalle de una orden de compra con sus partidas.
 */
@Schema(description = "Orden de compra con sus partidas ordenadas por número de línea.")
public record PurchaseOrderDetailResponse(PurchaseOrderResponse purchaseOrder, List<PurchaseOrderLineResponse> lines) {

    /**
     * Convierte el detalle de aplicación en su representación HTTP.
     *
     * @param detail orden con sus partidas
     * @return respuesta serializable del detalle
     */
    public static PurchaseOrderDetailResponse from(PurchaseOrderDetail detail) {
        return new PurchaseOrderDetailResponse(
                PurchaseOrderResponse.from(detail.order()),
                detail.lines().stream().map(PurchaseOrderLineResponse::from).toList());
    }
}
