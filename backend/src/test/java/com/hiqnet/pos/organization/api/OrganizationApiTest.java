package com.hiqnet.pos.organization.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.TestSecurityConfig;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.web.servlet.MockMvc;
import tools.jackson.databind.ObjectMapper;

/**
 * POST /organizations behavior (there is intentionally NO organization.create
 * permission; creating an organization will belong to secure bootstrap / the
 * Control Plane). Runs through the test-only chain because the endpoint has no
 * permission check; GET/PATCH organization behavior lives in
 * OrganizationAdminApiTest (real chain + RBAC).
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
@AutoConfigureMockMvc
class OrganizationApiTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private ObjectMapper objectMapper;

    private String body(String name, String code, String timezone, String currency, String locale) throws Exception {
        return objectMapper.writeValueAsString(new CreateOrganizationRequest(name, code, timezone, currency, locale));
    }

    private String createOrganization(String code) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("Cafeteria " + code, code, "America/Mexico_City", "MXN", "es-MX")))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createOrganizationReturns201AndDto() throws Exception {
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("Cafeteria Central", "first-org", "America/Mexico_City", "MXN", "es-MX")))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.name").value("Cafeteria Central"))
                .andExpect(jsonPath("$.code").value("first-org"))
                .andExpect(jsonPath("$.timezone").value("America/Mexico_City"))
                .andExpect(jsonPath("$.currency").value("MXN"))
                .andExpect(jsonPath("$.locale").value("es-MX"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty())
                .andExpect(jsonPath("$.updatedAt").isNotEmpty());
    }

    @Test
    void createOrganizationRejectsBlankName() throws Exception {
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("  ", "cafeteria", "America/Mexico_City", "MXN", "es-MX")))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void createOrganizationRejectsInvalidCode() throws Exception {
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("Cafeteria", "Bad Code!", "America/Mexico_City", "MXN", "es-MX")))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void createOrganizationRejectsDuplicateCode() throws Exception {
        createOrganization("cafeteria-central");
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("Cafeteria Central 2", "cafeteria-central", "America/Mexico_City", "MXN", "es-MX")))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("ORGANIZATION_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createOrganizationRejectsInvalidTimezone() throws Exception {
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("Cafeteria", "cafeteria", "Mars/Olympus", "MXN", "es-MX")))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_TIMEZONE"));
    }

    @Test
    void createOrganizationRejectsInvalidCurrency() throws Exception {
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("Cafeteria", "cafeteria", "America/Mexico_City", "XYZ", "es-MX")))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_CURRENCY"));
    }

    @Test
    void createOrganizationRejectsInvalidLocale() throws Exception {
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("Cafeteria", "cafeteria", "America/Mexico_City", "MXN", "not_a_locale")))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_LOCALE"));
    }

    @Test
    void createOrganizationTrimsNameWhitespace() throws Exception {
        mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body("   Cafeteria Central   ", "trim-create-org", "America/Mexico_City", "MXN", "es-MX")))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.name").value("Cafeteria Central"));
    }
}