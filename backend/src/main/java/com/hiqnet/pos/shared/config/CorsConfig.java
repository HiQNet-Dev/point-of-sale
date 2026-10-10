package com.hiqnet.pos.shared.config;

import java.util.Arrays;
import java.util.List;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.cors.CorsConfiguration;
import org.springframework.web.cors.CorsConfigurationSource;
import org.springframework.web.cors.UrlBasedCorsConfigurationSource;

/**
 * CORS is an explicit allowlist only (property {@code app.cors.allowed-origins}).
 * Empty by default: no cross-origin requests are allowed unless configured.
 *
 * <p>A wildcard origin ({@code *} or any value containing {@code *}, including
 * {@code https://*.example.com}) is REJECTED at startup: with no cookies/sessions
 * today (bearer tokens only) CORS is a UX convenience, never a security
 * boundary, and the documented contract explicitly forbids wildcard origins.
 * Fail fast beats a silently-broken assertion.</p>
 */
@Configuration
public class CorsConfig {

    private static final String WILDCARD = "*";

    @Bean
    public CorsConfigurationSource corsConfigurationSource(
            @Value("${app.cors.allowed-origins:}") String allowedOrigins) {
        List<String> origins = parseOrigins(allowedOrigins);

        UrlBasedCorsConfigurationSource source = new UrlBasedCorsConfigurationSource();
        if (!origins.isEmpty()) {
            CorsConfiguration configuration = new CorsConfiguration();
            configuration.setAllowedOrigins(origins);
            configuration.setAllowedMethods(List.of("GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"));
            configuration.setAllowedHeaders(List.of("Authorization", "Content-Type", "Accept"));
            configuration.setMaxAge(3600L);
            // allowCredentials deliberately stays false: the API uses bearer
            // tokens, not cookies; enabling credentials without a decision
            // would widen the trusted-request surface for no benefit.
            source.registerCorsConfiguration("/**", configuration);
        }
        return source;
    }

    /**
     * Normalizes the comma-separated list (trim, drop empties) and rejects any
     * wildcard. Public for testability; the bean fails fast on a wildcard.
     */
    static List<String> parseOrigins(String raw) {
        if (raw == null) {
            return List.of();
        }
        List<String> origins = Arrays.stream(raw.split(","))
                .map(String::trim)
                .filter(origin -> !origin.isEmpty())
                .toList();
        List<String> wildcards = origins.stream().filter(origin -> origin.contains(WILDCARD)).toList();
        if (!wildcards.isEmpty()) {
            throw new IllegalStateException(
                    "CORS_ALLOWED_ORIGINS must be an explicit allowlist without wildcards; found: "
                            + String.join(", ", wildcards));
        }
        return origins;
    }
}