package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.domain.Supplier;
import com.hiqnet.pos.purchasing.domain.SupplierStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import java.time.Instant;
import java.util.UUID;

/**
 * Representación pública de un proveedor scoped por organización.
 */
@Schema(description = "Proveedor registrado en una organización.")
public record SupplierResponse(
        @Schema(format = "uuid", example = "f4a0e638-d4ce-4f84-aa98-9d2a56e1dd3e") UUID id,
        @Schema(format = "uuid", example = "8f2da4d7-e5d7-4cee-8180-11cf3348907d") UUID organizationId,
        @Schema(example = "distribuidora-centro") String code,
        @Schema(example = "Distribuidora Centro") String name,
        @Schema(example = "Distribuidora Centro, S.A. de C.V.") String legalName,
        @Schema(example = "DCE010203AB4") String taxId,
        @Schema(example = "Ana López") String contactName,
        @Schema(example = "ventas@distribuidora-centro.example") String email,
        @Schema(example = "+52 55 5555 0101") String phone,
        @Schema(example = "Av. Central 100, Ciudad de México") String address,
        SupplierStatus status,
        @Schema(example = "0") long version,
        Instant createdAt,
        Instant updatedAt) {

    /**
     * Convierte el agregado de dominio en su representación HTTP.
     *
     * @param supplier proveedor persistido
     * @return respuesta serializable del proveedor
     */
    public static SupplierResponse from(Supplier supplier) {
        return new SupplierResponse(
                supplier.getId(),
                supplier.getOrganization().getId(),
                supplier.getCode(),
                supplier.getName(),
                supplier.getLegalName(),
                supplier.getTaxId(),
                supplier.getContactName(),
                supplier.getEmail(),
                supplier.getPhone(),
                supplier.getAddress(),
                supplier.getStatus(),
                supplier.getVersion(),
                supplier.getCreatedAt(),
                supplier.getUpdatedAt());
    }
}
