package com.hiqnet.pos.shared.config;

import com.hiqnet.pos.shared.error.ApiSecurityErrorHandler;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.HttpMethod;
import org.springframework.security.config.Customizer;
import org.springframework.security.config.annotation.method.configuration.EnableMethodSecurity;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.config.annotation.web.configuration.EnableWebSecurity;
import org.springframework.security.config.http.SessionCreationPolicy;
import org.springframework.security.web.SecurityFilterChain;
import org.springframework.security.web.access.intercept.AuthorizationFilter;
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
 * <p>Administrative APIs (organizations GET/PATCH, branches, users admin,
 * roles, permissions) require authentication here; the FINE-GRAINED
 * authorization lives in method security ({@code @PreAuthorize} calling
 * {@code AuthorizationService}, which is database-authoritative — the JWT is
 * identity only, no roles/permissions in the token). Matchers are explicit per
 * route: there is no blanket {@code /api/v1/**} rule and, critically, the
 * collection {@code POST /api/v1/organizations} stays DENIED (creating an
 * organization will belong to secure bootstrap / the Control Plane, not to a
 * permission).</p>
 */
@Configuration
@EnableWebSecurity
@EnableMethodSecurity
public class SecurityConfig {

    private static final String[] HEALTH_AND_DOCS = {"/actuator/health", "/error"};

    private static final String[] PUBLIC_AUTH = {
        "/api/v1/auth/login", "/api/v1/auth/refresh", "/api/v1/auth/logout"
    };

    @Bean
    public SecurityFilterChain securityFilterChain(
            HttpSecurity http,
            ObjectMapper objectMapper,
            BootstrapTokenGuard bootstrapGuard,
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
                        auth.requestMatchers("/v3/api-docs/**", "/swagger-ui.html", "/swagger-ui/**")
                                .permitAll();
                    }
                    auth.requestMatchers(HttpMethod.POST, PUBLIC_AUTH).permitAll();
                    // One-shot bootstrap: the route is reachable ONLY while a
                    // deployment secret is configured, and even then it is gated
                    // by BootstrapTokenFilter (verified before the body is read).
                    // With no secret configured the matcher is not added and the
                    // route falls through to anyRequest().denyAll().
                    if (bootstrapGuard.enabled()) {
                        auth.requestMatchers(HttpMethod.POST, "/api/v1/bootstrap").permitAll();
                    }
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/auth/logout-all").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/auth/me").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/auth/me/branches").authenticated();

                    // Administrative APIs: authentication required here; the
                    // fine-grained permission checks are in @PreAuthorize.
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/branches/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/memberships").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/memberships").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/memberships/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/branches/*/memberships/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/memberships/*/roles").authenticated();
                    auth.requestMatchers(HttpMethod.PUT, "/api/v1/organizations/*/branches/*/memberships/*/roles").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/users").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/users").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/users/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/users/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/users/*/roles").authenticated();
                    auth.requestMatchers(HttpMethod.PUT, "/api/v1/organizations/*/users/*/roles").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/roles").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/roles").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/roles/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/roles/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/roles/*/permissions").authenticated();
                    auth.requestMatchers(HttpMethod.PUT, "/api/v1/organizations/*/roles/*/permissions").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/permissions").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/audit-events").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/pos/catalog/categories").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/pos/catalog/products").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/pos/catalog/products/*/configuration").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/categories").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/categories").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/categories/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/categories/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/inventory-items").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/inventory-items").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/inventory-items/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/inventory-items/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/products").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/products").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/products/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/products/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/products/*/inventory-recipe").authenticated();
                    auth.requestMatchers(HttpMethod.PUT, "/api/v1/organizations/*/products/*/inventory-recipe").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/modifier-options/*/inventory-effects").authenticated();
                    auth.requestMatchers(HttpMethod.PUT, "/api/v1/organizations/*/modifier-options/*/inventory-effects").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/products/*/modifier-groups").authenticated();
                    auth.requestMatchers(HttpMethod.PUT, "/api/v1/organizations/*/products/*/modifier-groups").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/modifier-groups").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/modifier-groups").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/modifier-groups/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/modifier-groups/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/modifier-groups/*/options").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/modifier-groups/*/options").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/modifier-groups/*/options/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/modifier-groups/*/options/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/inventory-items").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/inventory-items/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/inventory-items/*/movements").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/inventory-items/*/opening-stock").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/inventory-items/*/adjustments").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/cash-registers").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/cash-registers/*").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/branches/*/cash-registers/*").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/close").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales/*").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales/*/checkout").authenticated();
                    auth.requestMatchers(HttpMethod.GET, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales/*/payments").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales/*/items").authenticated();
                    auth.requestMatchers(HttpMethod.PATCH, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales/*/items/*").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales/*/items/*/remove").authenticated();
                    auth.requestMatchers(HttpMethod.POST, "/api/v1/organizations/*/branches/*/cash-registers/*/cash-shifts/*/sales/*/cancel").authenticated();
                    auth.anyRequest().denyAll();
                })
                // Bootstrap token guard runs before authorization/body parsing:
                // a missing/invalid token never reaches the controller.
                .addFilterBefore(new BootstrapTokenFilter(bootstrapGuard, objectMapper), AuthorizationFilter.class)
                .oauth2ResourceServer(oauth2 -> oauth2
                        .jwt(Customizer.withDefaults())
                        // Route invalid/expired bearer tokens through our handler so
                        // the documented TOKEN_INVALID/TOKEN_EXPIRED JSON contract is
                        // honored; otherwise Spring's default bearer entry point wins
                        // and returns an empty 401 body.
                        .authenticationEntryPoint(errorHandler))
                .exceptionHandling(handling -> handling
                        .authenticationEntryPoint(errorHandler)
                        .accessDeniedHandler(errorHandler));

        return http.build();
    }
}