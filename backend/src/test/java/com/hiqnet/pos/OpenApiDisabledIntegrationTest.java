package com.hiqnet.pos;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.context.annotation.Import;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.web.servlet.MockMvc;

/**
 * OpenAPI exposure policy (default/closed). Without
 * {@code app.security.permit-opendoc=true}, the docs endpoints fall through the
 * deny-by-default chain of the REAL SecurityConfig. The production profile is
 * covered separately by {@link FoundationProdProfileTest}.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class})
@SpringBootTest
@AutoConfigureMockMvc
class OpenApiDisabledIntegrationTest {

    @Autowired
    private MockMvc mockMvc;

    @Test
    void docsAreNotExposedWhenTheFlagIsOff() throws Exception {
        mockMvc.perform(get("/v3/api-docs"))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
        mockMvc.perform(get("/swagger-ui/index.html"))
                .andExpect(status().isUnauthorized());
    }
}
