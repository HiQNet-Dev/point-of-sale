package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.application.PurchaseReceiptDetail;
import io.swagger.v3.oas.annotations.media.Schema;
import java.util.List;

/**
 * Respuesta de una recepción con sus líneas y la orden afectada.
 */
@Schema(description = "Recepción confirmada con sus líneas y el estado vigente de la orden.")
public record PurchaseReceiptDetailResponse(
        PurchaseReceiptResponse purchaseReceipt,
        List<PurchaseReceiptLineResponse> lines,
        PurchaseOrderResponse purchaseOrder) {

    /**
     * Convierte el detalle de aplicación en su representación HTTP.
     *
     * @param detail recepción con sus líneas y la orden
     * @return respuesta serializable del detalle
     */
    public static PurchaseReceiptDetailResponse from(PurchaseReceiptDetail detail) {
        return new PurchaseReceiptDetailResponse(
                PurchaseReceiptResponse.from(detail.receipt()),
                detail.lines().stream().map(PurchaseReceiptLineResponse::from).toList(),
                PurchaseOrderResponse.from(detail.purchaseOrder()));
    }
}
