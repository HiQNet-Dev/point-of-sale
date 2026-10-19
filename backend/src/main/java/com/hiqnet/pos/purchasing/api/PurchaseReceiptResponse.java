package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.domain.PurchaseReceipt;
import io.swagger.v3.oas.annotations.media.Schema;
import java.time.Instant;
import java.util.UUID;

/**
 * Representación pública de la cabecera de una recepción.
 */
@Schema(description = "Recepción confirmada e inmutable de una orden de compra.")
public record PurchaseReceiptResponse(
        @Schema(format = "uuid") UUID id,
        @Schema(format = "uuid") UUID organizationId,
        @Schema(format = "uuid") UUID branchId,
        @Schema(format = "uuid") UUID purchaseOrderId,
        @Schema(example = "receipt-2026-10-18-0001") String idempotencyKey,
        @Schema(format = "uuid") UUID receivedByUserId,
        Instant receivedAt,
        Instant createdAt) {

    /**
     * Convierte la recepción persistida en su representación HTTP.
     *
     * @param receipt recepción persistida
     * @return respuesta serializable de la cabecera
     */
    public static PurchaseReceiptResponse from(PurchaseReceipt receipt) {
        return new PurchaseReceiptResponse(
                receipt.getId(),
                receipt.getOrganizationId(),
                receipt.getBranchId(),
                receipt.getPurchaseOrderId(),
                receipt.getIdempotencyKey(),
                receipt.getReceivedByUserId(),
                receipt.getReceivedAt(),
                receipt.getCreatedAt());
    }
}
