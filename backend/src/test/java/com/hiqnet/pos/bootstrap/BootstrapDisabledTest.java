package com.hiqnet.pos.bootstrap;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.shared.config.BootstrapProperties;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.web.servlet.MockMvc;

/**
 * When no {@code BOOTSTRAP_SECRET} is configured the bootstrap route must stay
 * CLOSED (deny-by-default): the matcher is not added and the request falls
 * through to {@code anyRequest().denyAll()} without ever reaching the service.
 * The inline property override creates a dedicated context for this check.
 */
@ActiveProfiles("test")
@Import(TestcontainersConfiguration.class)
@SpringBootTest(properties = "app.bootstrap.secret=")
@AutoConfigureMockMvc
class BootstrapDisabledTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private BootstrapProperties bootstrapProperties;

    @Test
    void bootstrapRouteIsClosedWhenNoSecretIsConfigured() throws Exception {
        // The bootstrap guard reports disabled, so the service must be unreachable.
        org.assertj.core.api.Assertions.assertThat(bootstrapProperties.enabled()).isFalse();

        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", "c2VjcmV0LWtub3duLW9ubHktdG8tYS1jb25maWd1cmVkLXNlY3JldA==")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {"organization":{"name":"X","code":"x-1","timezone":"UTC","currency":"MXN","locale":"es-MX"},"administrator":{"email":"admin@example.com","password":"correct-horse-battery"}}
                                """))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
    }
}