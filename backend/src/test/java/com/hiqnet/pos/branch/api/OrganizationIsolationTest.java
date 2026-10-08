package com.hiqnet.pos.branch.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * CRITICAL isolation rule (future system convention):
 *
 * <pre>
 * Organization A -> Branch A1
 * Organization B -> Branch B1
 * </pre>
 *
 * A request for a branch id under the wrong organization must always return
 * 404, never 200 — prevents accidental IDOR between organizations. Runs under
 * the REAL security chain: the requester holds branch.read in its own
 * organization.
 */
class OrganizationIsolationTest extends AbstractRbacIntegrationTest {

    private String createBranch(String organizationId, String token, String code) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/branches")
                        .header("Authorization", bearer(token))
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
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        createBranch(fxA.orgId().toString(), tokenA, "a1");
        String branchB1 = createBranch(fxB.orgId().toString(), tokenB, "b1");

        mockMvc.perform(get("/api/v1/organizations/" + fxA.orgId() + "/branches/" + branchB1)
                        .header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void branchFromAnotherOrganizationIsNeverReturnedInReverseDirection() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String branchA1 = createBranch(fxA.orgId().toString(), tokenA, "a1");
        createBranch(fxB.orgId().toString(), tokenB, "b1");

        mockMvc.perform(get("/api/v1/organizations/" + fxB.orgId() + "/branches/" + branchA1)
                        .header("Authorization", bearer(tokenB)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void listOnlyReturnsBranchesOfRequestedOrganization() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        createBranch(fxA.orgId().toString(), tokenA, "a1");
        createBranch(fxB.orgId().toString(), tokenB, "b1");
        createBranch(fxB.orgId().toString(), tokenB, "b2");

        mockMvc.perform(get("/api/v1/organizations/" + fxA.orgId() + "/branches")
                        .header("Authorization", bearer(tokenA)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(1))
                .andExpect(jsonPath("$[0].code").value("a1"));
    }
}