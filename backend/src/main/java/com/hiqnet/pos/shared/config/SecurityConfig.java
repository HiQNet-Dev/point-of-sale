package com.hiqnet.pos.shared.config;

import com.hiqnet.pos.shared.error.ApiSecurityErrorHandler;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.security.config.Customizer;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.config.annotation.web.configuration.EnableWebSecurity;
import org.springframework.security.config.http.SessionCreationPolicy;
import org.springframework.security.web.SecurityFilterChain;
import tools.jackson.databind.ObjectMapper;

/**
 * Foundation security posture: deny by default.
 * Full authentication/authorization (users, JWT, roles) arrives with the auth
 * module. Business APIs under /api/v1/** are NEVER opened in production code:
 * test-only access to the real endpoints lives in the test sources
 * (com.hiqnet.pos.TestSecurityConfig) and is imported by the integration
 * tests. This file only ships the deny-by-default posture.
 */
@Configuration
@EnableWebSecurity
public class SecurityConfig {

    private static final String[] HEALTH_AND_DOCS = {"/actuator/health", "/error"};

    /**
     * Production/dev chain: deny everything except health, error and (only when
     * explicitly enabled) the OpenAPI docs. Never opens business APIs.
     */
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
                    auth.anyRequest().denyAll();
                })
                .exceptionHandling(handling -> handling
                        .authenticationEntryPoint(errorHandler)
                        .accessDeniedHandler(errorHandler));

        return http.build();
    }
}