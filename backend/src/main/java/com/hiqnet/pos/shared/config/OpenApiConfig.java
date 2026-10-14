package com.hiqnet.pos.shared.config;

import com.hiqnet.pos.shared.error.ApiError;
import io.swagger.v3.core.converter.ModelConverters;
import io.swagger.v3.oas.models.Components;
import io.swagger.v3.oas.models.OpenAPI;
import io.swagger.v3.oas.models.info.Info;
import io.swagger.v3.oas.models.media.Content;
import io.swagger.v3.oas.models.media.MediaType;
import io.swagger.v3.oas.models.media.Schema;
import io.swagger.v3.oas.models.responses.ApiResponse;
import io.swagger.v3.oas.models.security.SecurityRequirement;
import io.swagger.v3.oas.models.servers.Server;
import java.util.List;
import org.springdoc.core.customizers.OpenApiCustomizer;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * Metadata global de OpenAPI.
 *
 * <p>La estructura técnica del contrato (paths, requests, responses, schemas y
 * seguridad) la infiere springdoc a partir de los controllers, los DTOs y las
 * anotaciones {@code @Operation}/{@code @Schema}/{@code @SecurityRequirement}.
 * Este bean sólo aporta lo que springdoc no puede deducir: título, descripción
 * general, versión de la aplicación y el servidor local de desarrollo.</p>
 *
 * <p>La exposición de {@code /v3/api-docs} y {@code /swagger-ui} la decide
 * {@code app.security.permit-opendoc} en {@link SecurityConfig}; este bean no
 * abre ninguna ruta.</p>
 */
@Configuration
public class OpenApiConfig {

    @Bean
    public OpenAPI hiqNetOpenApi(@Value("${spring.application.version:}") String applicationVersion) {
        String version = (applicationVersion == null || applicationVersion.isBlank()
                        || applicationVersion.contains("@"))
                ? "development"
                : applicationVersion;
        return new OpenAPI()
                .info(new Info()
                        .title("HiQNet POS API")
                        .version(version)
                        .description("""
                                API REST para las operaciones del POS multisucursal HiQNet.

                                Las rutas de negocio están versionadas bajo `/api/v1` y son scoped por
                                organización (`/organizations/{organizationId}/...`). La autenticación usa
                                JWT Bearer y la autorización es por permisos (RBAC), nunca por roles fijos.
                                Ver `docs/03-api/README.md` para la guía de consumo desde el Frontend."""))
                .servers(List.of(new Server()
                        .url("http://localhost:8080")
                        .description("Backend local de desarrollo")))
                // Requisito de seguridad global: toda operación exige bearerAuth,
                // salvo las públicas (login/refresh/logout), que lo anulan con
                // @SecurityRequirements. El esquema lo declara AuthConfig.
                .addSecurityItem(new SecurityRequirement().addList("bearerAuth"));
    }

    private static final String API_ERROR_SCHEMA = "ApiError";

    private static final String JSON_MEDIA_TYPE = "application/json";

    /**
     * Expone la envoltura de error como un schema reutilizable del contrato.
     *
     * <p>Todas las respuestas de error del backend (4xx/5xx) comparten la forma
     * {@link ApiError}, pero los controllers declaran sus respuestas de error con
     * {@code content = @Content} (vacío) para no repetir la anotación en cada
     * operación. Sin este customizer el schema no aparece en
     * {@code /v3/api-docs} y el consumidor no ve la forma real del error.</p>
     *
     * <p>Aquí se registra {@code ApiError} en {@code components.schemas} y se
     * referencia desde cada respuesta 4xx/5xx (o {@code default}) que no declare
     * ya un contenido propio. Es puramente documental: no cambia el contrato en
     * runtime (el backend ya devuelve {@code ApiError} a través de
     * {@code GlobalExceptionHandler} y {@code ApiSecurityErrorHandler}).</p>
     */
    @Bean
    public OpenApiCustomizer apiErrorSchemaCustomizer() {
        return openApi -> {
            Components components = openApi.getComponents() != null ? openApi.getComponents() : new Components();
            openApi.setComponents(components);
            components.addSchemas(
                    API_ERROR_SCHEMA,
                    ModelConverters.getInstance()
                            .readAllAsResolvedSchema(ApiError.class)
                            .schema);

            if (openApi.getPaths() == null) {
                return;
            }
            openApi.getPaths().values().forEach(pathItem -> pathItem.readOperations().forEach(operation -> {
                if (operation.getResponses() == null) {
                    return;
                }
                operation.getResponses().forEach((status, response) -> attachApiError(status, response));
            }));
        };
    }

    private static void attachApiError(String status, ApiResponse response) {
        if (response == null || !isErrorStatus(status)) {
            return;
        }
        Content content = response.getContent();
        if (content != null && !content.isEmpty()) {
            return;
        }
        response.setContent(new Content()
                .addMediaType(
                        JSON_MEDIA_TYPE,
                        new MediaType().schema(new Schema<>().$ref("#/components/schemas/" + API_ERROR_SCHEMA))));
    }

    private static boolean isErrorStatus(String status) {
        if (status == null) {
            return false;
        }
        if ("default".equals(status)) {
            return true;
        }
        try {
            return Integer.parseInt(status) >= 400;
        } catch (NumberFormatException e) {
            return false;
        }
    }
}
