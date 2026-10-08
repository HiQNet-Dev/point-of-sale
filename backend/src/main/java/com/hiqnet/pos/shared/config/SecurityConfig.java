package com.hiqnet.pos.shared.config;

import com.hiqnet.pos.shared.error.ApiSecurityErrorHandler;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.HttpMethod;
import org.springframework.security.config.Customizer;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.config.annotation.web.configuration.EnableWebSecurity;
import org.springframework.security.config.http.SessionCreationPolicy;
import org.springframework.security.web.SecurityFilterChain;
import tools.jackson.databind.ObjectMapper;

/**
 * Deny-by-default security posture.
 *
 * <p>Public: health/error, and (only when explicitly enabled) OpenAPI docs.
 * Public auth endpoints: login, refresh and logout (logout carries the
 * refresh token in the body so it works even if the access token expired).
 * Authenticated: /me and logout-all (both require a valid Bearer access
 * token, validated as resource server: signature, expiry, issuer, audience,
 * token_type).</p>
 *
 * <p>Business APIs under /api/v1/** (organizations, branches, ...) stay
 * CLOSED in this task — there is no {@code authenticated()} wildcard that
 * would open them: any valid user could otherwise create/edit organizations
 * and branches, and Roles/Permissions do not exist yet.</p>
 *
 * <p>Test-only access to the real endpoints lives in the test sources
 * (com.hiqnet.pos.TestSecurityConfig) and is imported by legacy integration
 * tests; it can never be activated by a runtime profile.</p>
 */
@Configuration
@EnableWebSecurity
public class SecurityConfig {

    private static final String[] HEALTH_AND_DOCS = {"/actuator/health", "/error"};

    private static final String[] PUBLIC_AUTH = {
        "/api/v1/auth/login", "/api/v1/auth/refresh", "/api/v1/auth/logout"
    };

    @Bean
    public SecurityFilterChain securityFilterChain(
            HttpSecurity http,
            ObjectMapper objectMapper,
            @Value("${app.security.permit-opendoc:false}") boolean permitOpenDoc)
            throws Exception {
        ApiSecurityErrorHandler errorHandler = new ApiSecurityErrorHandler(objectMapper);

        http
                .cors(Customizer.withDefaults())
                .csrf(csrf -> csrf.disable())
                .sessionManagement(session -> session.sessionCreationPolicy(SessionCreationPolicy.STATELESS))
                .authorizeHttpRequests(auth -> {
                    auth.requestMatchers(HEALTH_AND_DOCS).permitAll();
                    if (permitOpenDoc) {
                        auth.requestMatchers("/v3/api-docs/**", "/swagger-ui/**").permitAll();
                    }
                    auth.requestMatchers(HttpMethod.POST, PUBLIC_AUTH).permitAll();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/auth/logout-all").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/auth/me").authenticated();
                    auth.anyRequest().denyAll();
                })
                .oauth2ResourceServer(oauth2 -> oauth2.jwt(Customizer.withDefaults()))
                .exceptionHandling(handling -> handling
                        .authenticationEntryPoint(errorHandler)
                        .accessDeniedHandler(errorHandler));

        return http.build();
    }
}