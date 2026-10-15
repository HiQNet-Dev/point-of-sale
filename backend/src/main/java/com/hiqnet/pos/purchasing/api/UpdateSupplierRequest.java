package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.domain.SupplierStatus;
import io.swagger.v3.oas.annotations.media.Schema;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

/**
 * Actualización parcial de un proveedor.
 *
 * <p>El código comercial no aparece porque es inmutable. Los campos omitidos o
 * {@code null} no cambian; para limpiar un texto opcional se envía una cadena
 * en blanco.</p>
 */
@Schema(description = "Campos editables de un proveedor. El código no puede cambiarse.")
public record UpdateSupplierRequest(
        @Schema(description = "Versión leída por el cliente; debe coincidir con la versión actual.", example = "0")
                @NotNull @Min(0) Long expectedVersion,
        @Schema(description = "Nuevo nombre comercial.", example = "Distribuidora Centro Norte")
                @Size(max = 120) String name,
        @Schema(description = "Nueva razón social; una cadena en blanco elimina el valor.", example = "Distribuidora Centro Norte, S.A. de C.V.")
                @Size(max = 160) String legalName,
        @Schema(description = "Nuevo identificador fiscal; una cadena en blanco elimina el valor.", example = "DCN010203AB4")
                @Size(max = 64) String taxId,
        @Schema(description = "Nuevo contacto principal; una cadena en blanco elimina el valor.", example = "Ana López")
                @Size(max = 120) String contactName,
        @Schema(description = "Nuevo correo; una cadena vacía elimina el valor.", example = "ventas@distribuidora-centro.example")
                @Size(max = 254) String email,
        @Schema(description = "Nuevo teléfono; una cadena en blanco elimina el valor.", example = "+52 55 5555 0102")
                @Size(max = 40) String phone,
        @Schema(description = "Nueva dirección; una cadena en blanco elimina el valor.", example = "Av. Central 200, Ciudad de México")
                @Size(max = 500) String address,
        @Schema(description = "Estado operativo; INACTIVE conserva el proveedor para referencias históricas.", example = "INACTIVE")
                SupplierStatus status) {
}
