package com.hiqnet.pos.branch.api;

import static org.hamcrest.Matchers.nullValue;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Branch administration under the REAL security chain + RBAC (branch.create /
 * branch.read / branch.update) and expectedVersion on PATCH.
 */
class BranchApiTest extends AbstractRbacIntegrationTest {

    private String createBranch(String organizationId, String token, String code, String timezone)
            throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(
                                new CreateBranchRequest("Sucursal " + code, code, timezone))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createBranchReturns201AndDto() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(
                                new CreateBranchRequest("Sucursal Centro", "centro", "America/Mexico_City"))))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.name").value("Sucursal Centro"))
                .andExpect(jsonPath("$.code").value("centro"))
                .andExpect(jsonPath("$.timezone").value("America/Mexico_City"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty());
    }

    @Test
    void createBranchWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createBranchOutsideReachableOrganizationIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + UUID.randomUUID() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createBranchRejectsDuplicateCodeWithinSameOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createBranch(fx.orgId().toString(), token, "centro", "America/Mexico_City");
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Otra Sucursal\",\"code\":\"centro\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BRANCH_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createBranchAllowsSameCodeAcrossOrganizations() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        createBranch(fxA.orgId().toString(), tokenA, "centro", null);
        mockMvc.perform(post("/api/v1/organizations/" + fxB.orgId() + "/branches")
                        .header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"centro\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.code").value("centro"));
    }

    @Test
    void createBranchRejectsInvalidTimezone() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\",\"timezone\":\"Not/AZone\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_TIMEZONE"));
    }

    @Test
    void createBranchAcceptsNullTimezone() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\",\"timezone\":null}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.timezone").value(nullValue()));
    }

    @Test
    void listBranchesReturnsAllForOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createBranch(fx.orgId().toString(), token, "norte", null);
        createBranch(fx.orgId().toString(), token, "sur", null);
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(2))
                .andExpect(jsonPath("$[0].organizationId").value(fx.orgId().toString()));
    }

    @Test
    void getBranchWithinOrganizationReturns200() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx.orgId().toString(), token, "centro", null);
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(branchId))
                .andExpect(jsonPath("$.code").value("centro"));
    }

    @Test
    void getBranchOfAnotherOrganizationReturns404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String branchB1 = createBranch(fxB.orgId().toString(), tokenB, "b1", null);

        // Organization A requesting Organization B's branch: never 200.
        mockMvc.perform(get("/api/v1/organizations/" + fxA.orgId() + "/branches/" + branchB1)
                        .header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void patchBranchUpdatesNameTimezoneAndStatus() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx.orgId().toString(), token, "centro", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Sucursal Centro Norte\","
                                + "\"timezone\":\"America/Argentina/Buenos_Aires\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Sucursal Centro Norte"))
                .andExpect(jsonPath("$.timezone").value("America/Argentina/Buenos_Aires"))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchBranchRejectsInvalidTimezone() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx.orgId().toString(), token, "centro", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"timezone\":\"Not/AZone\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_TIMEZONE"));
    }

    @Test
    void createBranchTrimsNameWhitespaceButKeepsInternalSpaces() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"   Sucursal   Centro   \",\"code\":\"trim-centro\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.name").value("Sucursal   Centro"));
    }

    @Test
    void patchBranchTrimsNameWhitespace() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx.orgId().toString(), token, "centro", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"  Sucursal Centro Renovada  \"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Sucursal Centro Renovada"));
    }

    @Test
    void patchBranchEmptyBodyKeepsTimezoneUnchanged() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx.orgId().toString(), token, "centro", "America/Mexico_City");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.timezone").value("America/Mexico_City"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void patchBranchNullTimezoneClearsOverrideAndInheritsOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx.orgId().toString(), token, "centro", "America/Argentina/Buenos_Aires");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"timezone\":null}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.timezone").value(nullValue()))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchBranchOfAnotherOrganizationReturns404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String branchB1 = createBranch(fxB.orgId().toString(), tokenB, "b1", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fxA.orgId() + "/branches/" + branchB1)
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void patchBranchStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx.orgId().toString(), token, "centro", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Sucursal Uno\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(1));
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }
}