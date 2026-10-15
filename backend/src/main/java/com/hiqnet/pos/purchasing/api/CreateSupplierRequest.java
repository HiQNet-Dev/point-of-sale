package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

/**
 * Datos para registrar un proveedor dentro de una organización.
 */
@Schema(description = "Datos necesarios para registrar un proveedor de la organización.")
public record CreateSupplierRequest(
        @Schema(description = "Código comercial inmutable, slug en minúsculas y único por organización.", example = "distribuidora-centro")
                @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @Schema(description = "Nombre comercial obligatorio.", example = "Distribuidora Centro")
                @NotBlank @Size(max = 120) String name,
        @Schema(description = "Razón social opcional.", example = "Distribuidora Centro, S.A. de C.V.")
                @Size(max = 160) String legalName,
        @Schema(description = "Identificador fiscal opcional.", example = "DCE010203AB4")
                @Size(max = 64) String taxId,
        @Schema(description = "Nombre del contacto principal opcional.", example = "Ana López")
                @Size(max = 120) String contactName,
        @Schema(description = "Correo de contacto opcional.", example = "ventas@distribuidora-centro.example")
                @Size(max = 254) String email,
        @Schema(description = "Teléfono de contacto opcional.", example = "+52 55 5555 0101")
                @Size(max = 40) String phone,
        @Schema(description = "Dirección opcional.", example = "Av. Central 100, Ciudad de México")
                @Size(max = 500) String address) {
}
