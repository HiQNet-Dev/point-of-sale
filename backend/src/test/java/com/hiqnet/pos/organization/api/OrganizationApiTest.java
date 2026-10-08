package com.hiqnet.pos.organization.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.TestcontainersConfiguration;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.web.servlet.MockMvc;
import tools.jackson.databind.ObjectMapper;

@ActiveProfiles("test")
@Import(TestcontainersConfiguration.class)
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
    void getOrganizationReturns200() throws Exception {
        String id = createOrganization("get-me");
        mockMvc.perform(get("/api/v1/organizations/" + id))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(id))
                .andExpect(jsonPath("$.code").value("get-me"));
    }

    @Test
    void getOrganizationReturns404() throws Exception {
        mockMvc.perform(get("/api/v1/organizations/" + UUID.randomUUID()))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ORGANIZATION_NOT_FOUND"));
    }

    @Test
    void patchOrganizationUpdatesPermittedFieldsAndBumpsVersion() throws Exception {
        String id = createOrganization("patch-me");
        mockMvc.perform(patch("/api/v1/organizations/" + id)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{ \"name\": \"Cafe Renovado\", \"timezone\": \"America/Argentina/Buenos_Aires\", \"currency\": \"USD\", \"locale\": \"en-US\" }"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Cafe Renovado"))
                .andExpect(jsonPath("$.timezone").value("America/Argentina/Buenos_Aires"))
                .andExpect(jsonPath("$.currency").value("USD"))
                .andExpect(jsonPath("$.locale").value("en-US"))
                .andExpect(jsonPath("$.version").value(1));

        // PATCH never modifies the immutable code.
        mockMvc.perform(get("/api/v1/organizations/" + id))
                .andExpect(jsonPath("$.code").value("patch-me"));
    }

    @Test
    void patchOrganizationEmptyBodyIsNoOp() throws Exception {
        String id = createOrganization("noop-me");
        mockMvc.perform(patch("/api/v1/organizations/" + id)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void patchOrganizationDeactivatesAndActivates() throws Exception {
        String id = createOrganization("deactivate-me");
        mockMvc.perform(patch("/api/v1/organizations/" + id)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{ \"status\": \"INACTIVE\" }"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("INACTIVE"));

        mockMvc.perform(patch("/api/v1/organizations/" + id)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{ \"status\": \"ACTIVE\" }"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("ACTIVE"));
    }

    @Test
    void patchOrganizationRejectsInvalidValues() throws Exception {
        String id = createOrganization("patch-invalid");
        mockMvc.perform(patch("/api/v1/organizations/" + id)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{ \"timezone\": \"Not/AZone\" }"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_TIMEZONE"));
    }
}