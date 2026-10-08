package com.hiqnet.pos.branch.api;

import static org.hamcrest.Matchers.nullValue;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.organization.api.CreateOrganizationRequest;
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
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
@AutoConfigureMockMvc
class BranchApiTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private ObjectMapper objectMapper;

    private String createOrganization(String code) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(
                                new CreateOrganizationRequest(code + " Org", code, "America/Mexico_City", "MXN", "es-MX"))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createBranch(String organizationId, String code, String timezone) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(new CreateBranchRequest("Sucursal " + code, code, timezone))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createBranchReturns201AndDto() throws Exception {
        String orgId = createOrganization("branch-org-a");
        mockMvc.perform(post("/api/v1/organizations/" + orgId + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(new CreateBranchRequest("Sucursal Centro", "centro", "America/Mexico_City"))))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(orgId))
                .andExpect(jsonPath("$.name").value("Sucursal Centro"))
                .andExpect(jsonPath("$.code").value("centro"))
                .andExpect(jsonPath("$.timezone").value("America/Mexico_City"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty());
    }

    @Test
    void createBranchRejectsUnknownOrganization() throws Exception {
        mockMvc.perform(post("/api/v1/organizations/" + UUID.randomUUID() + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ORGANIZATION_NOT_FOUND"));
    }

    @Test
    void createBranchRejectsDuplicateCodeWithinSameOrganization() throws Exception {
        String orgId = createOrganization("branch-dup-org");
        createBranch(orgId, "centro", "America/Mexico_City");
        mockMvc.perform(post("/api/v1/organizations/" + orgId + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Otra Sucursal\",\"code\":\"centro\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BRANCH_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createBranchAllowsSameCodeAcrossOrganizations() throws Exception {
        String orgA = createOrganization("dup-org-a");
        String orgB = createOrganization("dup-org-b");
        createBranch(orgA, "centro", null);
        mockMvc.perform(post("/api/v1/organizations/" + orgB + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"centro\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.code").value("centro"));
    }

    @Test
    void createBranchRejectsInvalidTimezone() throws Exception {
        String orgId = createOrganization("branch-tz-org");
        mockMvc.perform(post("/api/v1/organizations/" + orgId + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\",\"timezone\":\"Not/AZone\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_TIMEZONE"));
    }

    @Test
    void createBranchAcceptsNullTimezone() throws Exception {
        String orgId = createOrganization("branch-null-tz");
        mockMvc.perform(post("/api/v1/organizations/" + orgId + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\",\"timezone\":null}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.timezone").value(nullValue()));
    }

    @Test
    void listBranchesReturnsAllForOrganization() throws Exception {
        String orgId = createOrganization("branch-list-org");
        createBranch(orgId, "norte", null);
        createBranch(orgId, "sur", null);
        mockMvc.perform(get("/api/v1/organizations/" + orgId + "/branches"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(2))
                .andExpect(jsonPath("$[0].organizationId").value(orgId))
                .andExpect(jsonPath("$[1].organizationId").value(orgId));
    }

    @Test
    void getBranchWithinOrganizationReturns200() throws Exception {
        String orgId = createOrganization("branch-get-org");
        String branchId = createBranch(orgId, "centro", null);
        mockMvc.perform(get("/api/v1/organizations/" + orgId + "/branches/" + branchId))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(branchId))
                .andExpect(jsonPath("$.code").value("centro"));
    }

    @Test
    void getBranchOfAnotherOrganizationReturns404() throws Exception {
        String orgA = createOrganization("isolation-a");
        String orgB = createOrganization("isolation-b");
        String branchB1 = createBranch(orgB, "b1", null);

        // Organization A requesting Organization B's branch: never 200.
        mockMvc.perform(get("/api/v1/organizations/" + orgA + "/branches/" + branchB1))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void patchBranchUpdatesNameTimezoneAndStatus() throws Exception {
        String orgId = createOrganization("branch-patch-org");
        String branchId = createBranch(orgId, "centro", null);
        mockMvc.perform(patch("/api/v1/organizations/" + orgId + "/branches/" + branchId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal Centro Norte\",\"timezone\":\"America/Argentina/Buenos_Aires\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Sucursal Centro Norte"))
                .andExpect(jsonPath("$.timezone").value("America/Argentina/Buenos_Aires"))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchBranchRejectsInvalidTimezone() throws Exception {
        String orgId = createOrganization("branch-patch-tz");
        String branchId = createBranch(orgId, "centro", null);
        mockMvc.perform(patch("/api/v1/organizations/" + orgId + "/branches/" + branchId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"timezone\":\"Not/AZone\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_TIMEZONE"));
    }

    @Test
    void createBranchTrimsNameWhitespaceButKeepsInternalSpaces() throws Exception {
        String orgId = createOrganization("branch-trim-create");
        mockMvc.perform(post("/api/v1/organizations/" + orgId + "/branches")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"   Sucursal   Centro   \",\"code\":\"trim-centro\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.name").value("Sucursal   Centro"));
    }

    @Test
    void patchBranchTrimsNameWhitespace() throws Exception {
        String orgId = createOrganization("branch-trim-patch");
        String branchId = createBranch(orgId, "centro", null);
        mockMvc.perform(patch("/api/v1/organizations/" + orgId + "/branches/" + branchId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"  Sucursal Centro Renovada  \"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Sucursal Centro Renovada"));
    }

    @Test
    void patchBranchEmptyBodyKeepsTimezoneUnchanged() throws Exception {
        String orgId = createOrganization("branch-patch-noop");
        String branchId = createBranch(orgId, "centro", "America/Mexico_City");
        mockMvc.perform(patch("/api/v1/organizations/" + orgId + "/branches/" + branchId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.timezone").value("America/Mexico_City"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void patchBranchNullTimezoneClearsOverrideAndInheritsOrganization() throws Exception {
        String orgId = createOrganization("branch-tz-clear");
        String branchId = createBranch(orgId, "centro", "America/Argentina/Buenos_Aires");
        mockMvc.perform(patch("/api/v1/organizations/" + orgId + "/branches/" + branchId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"timezone\":null}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.timezone").value(nullValue()))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchBranchOfAnotherOrganizationReturns404() throws Exception {
        String orgA = createOrganization("patch-isolation-a");
        String orgB = createOrganization("patch-isolation-b");
        String branchB1 = createBranch(orgB, "b1", null);
        mockMvc.perform(patch("/api/v1/organizations/" + orgA + "/branches/" + branchB1)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Hack\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }
}