package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;
import java.util.UUID;

/**
 * Datos para crear una orden de compra en borrador.
 */
@Schema(description = "Datos necesarios para crear una orden de compra en borrador.")
public record CreatePurchaseOrderRequest(
        @Schema(description = "Código comercial inmutable, slug en minúsculas y único por organización.", example = "po-2026-0001")
                @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @Schema(description = "Proveedor de la organización, activo.", example = "f4a0e638-d4ce-4f84-aa98-9d2a56e1dd3e")
                @NotNull UUID supplierId) {
}
