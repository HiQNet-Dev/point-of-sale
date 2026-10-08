package com.hiqnet.pos;

import com.hiqnet.pos.shared.error.ApiSecurityErrorHandler;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.core.annotation.Order;
import org.springframework.security.config.Customizer;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.config.http.SessionCreationPolicy;
import org.springframework.security.web.SecurityFilterChain;
import tools.jackson.databind.ObjectMapper;

/**
 * Test-only security configuration: exposes the real /api/v1/** endpoints so
 * integration tests can exercise controllers end-to-end without an
 * authentication provider. Lives exclusively in the test sources; the
 * production artifact only contains the deny-by-default chain from
 * {@code SecurityConfig}. Each Spring Boot test imports this configuration
 * explicitly; it can never be activated by a runtime profile.
 */
@TestConfiguration(proxyBeanMethods = false)
public class TestSecurityConfig {

    @Bean
    @Order(0)
    SecurityFilterChain testSecurityFilterChain(HttpSecurity http, ObjectMapper objectMapper) throws Exception {
        ApiSecurityErrorHandler errorHandler = new ApiSecurityErrorHandler(objectMapper);

        http
                .securityMatcher("/actuator/health", "/error", "/v3/api-docs/**", "/swagger-ui/**", "/api/v1/**")
                .cors(Customizer.withDefaults())
                .csrf(csrf -> csrf.disable())
                .sessionManagement(session -> session.sessionCreationPolicy(SessionCreationPolicy.STATELESS))
                .authorizeHttpRequests(auth -> auth
                        .requestMatchers("/actuator/health", "/error", "/v3/api-docs/**", "/swagger-ui/**")
                        .permitAll()
                        .requestMatchers("/api/v1/**")
                        .permitAll()
                        .anyRequest()
                        .denyAll())
                .exceptionHandling(handling -> handling
                        .authenticationEntryPoint(errorHandler)
                        .accessDeniedHandler(errorHandler));

        return http.build();
    }
}