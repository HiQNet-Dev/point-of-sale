package com.hiqnet.pos.shared.config;

import io.swagger.v3.oas.models.OpenAPI;
import io.swagger.v3.oas.models.info.Info;
import io.swagger.v3.oas.models.servers.Server;
import java.util.List;
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
                        .description("Backend local de desarrollo")));
    }
}
