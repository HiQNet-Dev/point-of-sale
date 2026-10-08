package com.hiqnet.pos.branch.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.organization.api.CreateOrganizationRequest;
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
 * CRITICAL isolation rule (future system convention):
 *
 * <pre>
 * Organization A -> Branch A1
 * Organization B -> Branch B1
 * </pre>
 *
 * A request for a branch id under the wrong organization must always return
 * 404, never 200 — prevents accidental IDOR between organizations.
 */
@ActiveProfiles("test")
@Import(TestcontainersConfiguration.class)
@SpringBootTest
@AutoConfigureMockMvc
class OrganizationIsolationTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private ObjectMapper objectMapper;

    private String createOrganization(String code) throws Exception {
        String response = mockMvc.perform(org.springframework.test.web.servlet.request.MockMvcRequestBuilders
                        .post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(
                                new CreateOrganizationRequest(code + " Org", code, "America/Mexico_City", "MXN", "es-MX"))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createBranch(String organizationId, String code) throws Exception {
        String response = mockMvc.perform(org.springframework.test.web.servlet.request.MockMvcRequestBuilders
                        .post("/api/v1/organizations/" + organizationId + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"" + code + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void branchFromAnotherOrganizationIsNeverReturned() throws Exception {
        String orgA = createOrganization("iso-a");
        createBranch(orgA, "a1");
        String orgB = createOrganization("iso-b");
        String branchB1 = createBranch(orgB, "b1");

        mockMvc.perform(get("/api/v1/organizations/" + orgA + "/branches/" + branchB1))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void branchFromAnotherOrganizationIsNeverReturnedInReverseDirection() throws Exception {
        String orgA = createOrganization("iso-a-rev");
        String branchA1 = createBranch(orgA, "a1");
        String orgB = createOrganization("iso-b-rev");
        createBranch(orgB, "b1");

        mockMvc.perform(get("/api/v1/organizations/" + orgB + "/branches/" + branchA1))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void listOnlyReturnsBranchesOfRequestedOrganization() throws Exception {
        String orgA = createOrganization("iso-list-a");
        createBranch(orgA, "a1");
        String orgB = createOrganization("iso-list-b");
        createBranch(orgB, "b1");
        createBranch(orgB, "b2");

        mockMvc.perform(get("/api/v1/organizations/" + orgA + "/branches"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(1))
                .andExpect(jsonPath("$[0].code").value("a1"));
    }
}