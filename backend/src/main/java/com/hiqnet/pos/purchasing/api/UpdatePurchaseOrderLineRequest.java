package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import tools.jackson.databind.JsonNode;

/**
 * Actualización parcial de una partida de un borrador.
 *
 * <p>Un campo ausente no se modifica y {@code quantity} {@code null} tampoco. El
 * costo unitario distingue "sin cambios" de "limpiar" con la semántica tri-estado
 * de {@code PatchField}: omitirlo lo deja igual y un {@code null} explícito lo
 * limpia a {@code NULL}. El artículo y sus snapshots son inmutables: para cambiar
 * de artículo se elimina la partida y se agrega otra.</p>
 */
@Schema(description = "Campos editables de una partida. El artículo y sus snapshots no pueden cambiarse.")
public record UpdatePurchaseOrderLineRequest(
        @Schema(description = "Versión del agregado leída por el cliente.", example = "1")
                @NotNull @Min(0) Long expectedVersion,
        @Schema(
                        type = "string",
                        example = "26000.000000",
                        description = "Nueva cantidad opcional, como string decimal estrictamente positivo.")
                JsonNode quantity,
        @Schema(
                        type = "string",
                        nullable = true,
                        example = "0.0190",
                        description = "Nuevo costo unitario como string decimal no negativo. Omitir para no "
                                + "cambiarlo; `null` explícito limpia el costo a NULL; `0.0000` asigna cero.")
                JsonNode unitCost) {
}
