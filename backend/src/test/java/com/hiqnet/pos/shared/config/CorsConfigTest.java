package com.hiqnet.pos.shared.config;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import org.junit.jupiter.api.Test;
import org.springframework.boot.test.context.runner.ApplicationContextRunner;
import org.springframework.mock.web.MockHttpServletRequest;
import org.springframework.web.cors.CorsConfiguration;
import org.springframework.web.cors.CorsConfigurationSource;

/**
 * CORS is an explicit allowlist: empty means "no cross-origin requests", a
 * comma-separated list is normalized, and a wildcard origin ({@code *} or any
 * origin containing {@code *}) is rejected — at configuration time, so a
 * misconfigured deployment fails to start instead of silently allowing any
 * origin. Credentials are never enabled (bearer API, no cookies).
 */
class CorsConfigTest {

    private final ApplicationContextRunner contextRunner =
            new ApplicationContextRunner().withUserConfiguration(CorsConfig.class);

    @Test
    void emptyValueProducesNoCorsMapping() {
        assertThat(CorsConfig.parseOrigins("")).isEmpty();
        assertThat(CorsConfig.parseOrigins(" , , ")).isEmpty();
        assertThat(CorsConfig.parseOrigins(null)).isEmpty();

        contextRunner
                .withPropertyValues("app.cors.allowed-origins=")
                .run(context -> {
                    CorsConfigurationSource source = context.getBean(CorsConfigurationSource.class);
                    assertThat(source.getCorsConfiguration(request("/api/v1/anything"))).isNull();
                });
    }

    @Test
    void singleOriginIsNormalizedAndRegistered() {
        contextRunner
                .withPropertyValues("app.cors.allowed-origins= https://a.example.com ")
                .run(context -> {
                    CorsConfiguration configuration =
                            context.getBean(CorsConfigurationSource.class).getCorsConfiguration(request("/api/v1/"));
                    assertThat(configuration.getAllowedOrigins()).containsExactly("https://a.example.com");
                    // Bearer-only API: credentials are intentionally disabled
                    // (null = Spring's default "credentials not allowed").
                    assertThat(configuration.getAllowCredentials()).isNull();
                });
    }

    @Test
    void multipleOriginsAreTrimmedIndividually() {
        contextRunner
                .withPropertyValues("app.cors.allowed-origins=https://a.example.com, https://b.example.com")
                .run(context -> {
                    CorsConfiguration configuration =
                            context.getBean(CorsConfigurationSource.class).getCorsConfiguration(request("/api/v1/"));
                    assertThat(configuration.getAllowedOrigins())
                            .containsExactly("https://a.example.com", "https://b.example.com");
                });
    }

    @Test
    void plainWildcardOriginIsRejected() {
        assertThatThrownBy(() -> CorsConfig.parseOrigins("*"))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("without wildcards");
    }

    @Test
    void wildcardSubdomainPatternIsRejected() {
        // Spring can interpret patterns containing '*' as wildcard origins;
        // the explicit-allowlist contract forbids them all.
        assertThatThrownBy(() -> CorsConfig.parseOrigins("https://*.example.com"))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("without wildcards");
    }

    @Test
    void wildcardOriginFailsApplicationStartup() {
        contextRunner
                .withPropertyValues("app.cors.allowed-origins=*")
                .run(context -> assertThat(context).hasFailed());
    }

    private static MockHttpServletRequest request(String requestUri) {
        MockHttpServletRequest request = new MockHttpServletRequest();
        request.setRequestURI(requestUri);
        return request;
    }
}