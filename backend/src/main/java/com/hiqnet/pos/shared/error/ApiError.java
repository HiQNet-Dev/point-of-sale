package com.hiqnet.pos.shared.error;

import io.swagger.v3.oas.annotations.media.Schema;
import java.time.Instant;
import java.util.List;

/**
 * Stable API error contract. Never includes stack traces or internal details.
 *
 * <p>Se expone como schema reutilizable de OpenAPI en {@code OpenApiConfig}
 * ({@code components.schemas.ApiError}) y es la forma referenciada por toda
 * respuesta 4xx/5xx.</p>
 */
@Schema(description = "Envoltura estable de error. Presente en toda respuesta 4xx/5xx.")
public record ApiError(
        @Schema(description = "Instante UTC en ISO-8601.", example = "2026-10-07T12:00:00Z") Instant timestamp,
        @Schema(description = "Espejo del código HTTP.", example = "409") int status,
        @Schema(
                        description = "Código estable. El cliente debe conmutar por este valor, nunca por message.",
                        example = "BRANCH_CODE_ALREADY_EXISTS")
                String code,
        @Schema(description = "Mensaje legible y genérico; no es verdad de negocio.", example = "A branch with this code already exists")
                String message,
        @Schema(description = "Ruta solicitada.", example = "/api/v1/organizations/.../branches") String path,
        @Schema(
                        description = "Detalles opcionales (p. ej. errores de validación de campos). Puede ser un array vacío.",
                        example = "[]")
                List<String> details) {

    public static ApiError of(int status, String code, String message, String path) {
        return new ApiError(Instant.now(), status, code, message, path, List.of());
    }

    public static ApiError of(int status, String code, String message, String path, List<String> details) {
        return new ApiError(Instant.now(), status, code, message, path, details);
    }
}
