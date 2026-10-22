package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import java.time.Instant;
import java.util.UUID;

/**
 * Representación pública de la cabecera de una orden de compra.
 */
@Schema(description = "Orden de compra de una sucursal, con snapshots del proveedor.")
public record PurchaseOrderResponse(
        @Schema(format = "uuid", example = "9c1d2e3f-4a5b-4c6d-8e7f-0a1b2c3d4e5f") UUID id,
        @Schema(format = "uuid", example = "8f2da4d7-e5d7-4cee-8180-11cf3348907d") UUID organizationId,
        @Schema(format = "uuid", example = "3a7c1b90-1f2e-4c3d-9a4b-5c6d7e8f9012") UUID branchId,
        @Schema(format = "uuid", example = "f4a0e638-d4ce-4f84-aa98-9d2a56e1dd3e") UUID supplierId,
        @Schema(example = "distribuidora-centro") String supplierCodeSnapshot,
        @Schema(example = "Distribuidora Centro") String supplierNameSnapshot,
        @Schema(example = "po-2026-0001") String code,
        PurchaseOrderStatus status,
        @Schema(example = "MXN", description = "Moneda ISO 4217 copiada de la organización; el cliente no puede fijarla.") String currency,
        @Schema(example = "0") long version,
        @Schema(format = "uuid", example = "0b6f2a11-2c3d-4e5f-8a9b-0c1d2e3f4a5b") UUID createdByUserId,
        @Schema(format = "uuid") UUID issuedByUserId,
        @Schema(format = "uuid") UUID cancelledByUserId,
        Instant createdAt,
        Instant updatedAt,
        Instant issuedAt,
        Instant cancelledAt) {

    /**
     * Convierte la cabecera persistida en su representación HTTP.
     *
     * @param order orden persistida
     * @return respuesta serializable de la cabecera
     */
    public static PurchaseOrderResponse from(PurchaseOrder order) {
        return new PurchaseOrderResponse(
                order.getId(),
                order.getOrganization().getId(),
                order.getBranchId(),
                order.getSupplier().getId(),
                order.getSupplierCodeSnapshot(),
                order.getSupplierNameSnapshot(),
                order.getCode(),
                order.getStatus(),
                order.getCurrency(),
                order.getVersion(),
                order.getCreatedByUserId(),
                order.getIssuedByUserId(),
                order.getCancelledByUserId(),
                order.getCreatedAt(),
                order.getUpdatedAt(),
                order.getIssuedAt(),
                order.getCancelledAt());
    }
}
