package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.Valid;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotEmpty;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;
import java.util.List;

/**
 * Datos para registrar una recepción de mercancía.
 */
@Schema(description = "Recepción de mercancía de una orden emitida; idempotente por `idempotencyKey`.")
public record PostPurchaseReceiptRequest(
        @Schema(description = "Versión de la orden leída por el cliente.", example = "2")
                @NotNull @Min(0) Long expectedVersion,
        @Schema(
                        description = "Clave de idempotencia única por organización y orden. Un reintento con el mismo "
                                + "contenido devuelve el resultado original; con contenido distinto responde 409.",
                        example = "receipt-2026-10-18-0001")
                @NotNull @Size(min = 1, max = 128) String idempotencyKey,
        @Schema(description = "Cantidades recibidas por partida; al menos una.")
                @NotEmpty @Valid List<PostPurchaseReceiptLineRequest> lines) {
}
