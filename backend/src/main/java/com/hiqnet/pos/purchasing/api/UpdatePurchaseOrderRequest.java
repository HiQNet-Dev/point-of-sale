package com.hiqnet.pos.purchasing.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;

/**
 * Actualización parcial de la cabecera de un borrador.
 *
 * <p>El estado y la moneda no aparecen: el estado solo cambia por transiciones de
 * dominio y la moneda la determina el servidor.</p>
 */
@Schema(description = "Campos editables de la cabecera. El código, la moneda y el estado no pueden cambiarse.")
public record UpdatePurchaseOrderRequest(
        @Schema(description = "Versión leída por el cliente; debe coincidir con la versión actual.", example = "0")
                @NotNull @Min(0) Long expectedVersion,
        @Schema(description = "Nuevo proveedor activo de la organización; omitir para no cambiarlo.", example = "f4a0e638-d4ce-4f84-aa98-9d2a56e1dd3e")
                UUID supplierId) {
}
