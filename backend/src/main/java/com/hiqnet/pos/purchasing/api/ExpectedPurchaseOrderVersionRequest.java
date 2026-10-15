package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;

/**
 * Cuerpo con la versión esperada para operaciones que no llevan otros datos.
 */
@Schema(description = "Versión del agregado leída por el cliente.")
public record ExpectedPurchaseOrderVersionRequest(
        @Schema(description = "Versión leída por el cliente; debe coincidir con la versión actual.", example = "1")
                @NotNull @Min(0) Long expectedVersion) {
}
