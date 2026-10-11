package com.hiqnet.pos;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.shared.config.BootstrapProperties;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.context.DynamicPropertyRegistry;
import org.springframework.test.context.DynamicPropertySource;
import org.springframework.test.web.servlet.MockMvc;
import org.testcontainers.containers.PostgreSQLContainer;
import org.testcontainers.junit.jupiter.Container;
import org.testcontainers.junit.jupiter.Testcontainers;

/**
 * Production-profile smoke: boots the PRODUCTION configuration (real
 * {@code SecurityConfig}, {@code application-prod.yml}) against a real
 * PostgreSQL, providing the deployment variables exactly as an operator would
 * (DB_URL/DB_USER/DB_PASSWORD/AUTH_JWT_SECRET) — but NO {@code BOOTSTRAP_SECRET}.
 *
 * <p>Verifies the production posture without external infrastructure: the
 * application starts with Flyway applied (V1-V11), OpenAPI stays closed, the
 * bootstrap route is denied (no secret configured), and health is UP.</p>
 */
@ActiveProfiles("prod")
@Testcontainers
@SpringBootTest
@AutoConfigureMockMvc
class FoundationProdProfileTest {

    private static final String TEST_JWT_SECRET = "aGlxbmV0LXBvcy10ZXN0LW9ubHktand0LXNpZ25pbmcta2V5";

    @Container
    static final PostgreSQLContainer<?> POSTGRES = new PostgreSQLContainer<>("postgres:17-alpine");

    @DynamicPropertySource
    static void prodDeploymentVariables(DynamicPropertyRegistry registry) {
        // Exactly what a production deployment operator provides (no defaults).
        registry.add("DB_URL", POSTGRES::getJdbcUrl);
        registry.add("DB_USER", POSTGRES::getUsername);
        registry.add("DB_PASSWORD", POSTGRES::getPassword);
        registry.add("AUTH_JWT_SECRET", () -> TEST_JWT_SECRET);
        // BOOTSTRAP_SECRET is intentionally absent: bootstrap must be DISABLED.
    }

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private BootstrapProperties bootstrapProperties;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Value("${app.security.permit-opendoc:true}")
    private boolean permitOpenDoc;

    @Test
    void productionPostureIsVerified() throws Exception {
        // Fail-fast free-form secrets validated; OpenAPI closed by default.
        assertThat(permitOpenDoc).isFalse();
        // No bootstrap secret configured -> bootstrap HTTP disabled.
        assertThat(bootstrapProperties.enabled()).isFalse();

        mockMvc.perform(get("/actuator/health"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("UP"));

        // OpenAPI is NOT reachable in production.
        mockMvc.perform(get("/v3/api-docs"))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
        mockMvc.perform(get("/swagger-ui/index.html"))
                .andExpect(status().isUnauthorized());

        // Bootstrap route is closed (no matcher) and nothing was provisioned.
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", "c2VjcmV0LWtub3duLW9ubHktdG8tYS1jb25maWd1cmVkLXNlY3JldA==")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {"organization":{"name":"X","code":"x-1","timezone":"UTC","currency":"MXN","locale":"es-MX"},"administrator":{"email":"admin@example.com","password":"correct-horse-battery"}}
                                """))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));

        // Migrations applied cleanly on the fresh DB and nothing was created.
        Long organizations = jdbcTemplate.queryForObject("select count(*) from organizations", Long.class);
        assertThat(organizations).isZero();
        Long permissions = jdbcTemplate.queryForObject("select count(*) from permissions", Long.class);
        assertThat(permissions).isEqualTo(42L);
        Boolean completed = jdbcTemplate.queryForObject(
                "select completed_at is not null from installation_bootstrap where id = 1", Boolean.class);
        assertThat(completed).isFalse();
    }
}