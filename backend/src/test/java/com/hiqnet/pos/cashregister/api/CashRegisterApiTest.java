package com.hiqnet.pos.cashregister.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Cash register administration under the REAL security chain + RBAC
 * (cash-register.create / read / update): code is an immutable lowercase slug
 * unique per branch, PATCH is partial, and every lookup is scoped to the
 * organization AND branch.
 */
class CashRegisterApiTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    private UUID createBranch(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository
                .saveAndFlush(Branch.create(organization, "Branch " + code, code, null))
                .getId();
    }

    private String createRegister(RbacFixture fx, UUID branchId, String token, String code, String name)
            throws Exception {
        String response = mockMvc.perform(post(url(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(new CreateCashRegisterRequest(code, name))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private static String url(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }

    @Test
    void createCashRegisterReturns201WithServerAssignedFields() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");

        mockMvc.perform(post(url(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front Desk\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.branchId").value(branchId.toString()))
                .andExpect(jsonPath("$.code").value("register-1"))
                .andExpect(jsonPath("$.name").value("Front Desk"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void createCashRegisterWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        UUID branchId = createBranch(fx, "centro");

        mockMvc.perform(post(url(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front Desk\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createCashRegisterRejectsDuplicateCodeWithinSameBranch() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        createRegister(fx, branchId, token, "register-1", "Front Desk");

        mockMvc.perform(post(url(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Other\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CASH_REGISTER_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createCashRegisterAllowsSameCodeInAnotherBranchOfSameOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchA = createBranch(fx, "centro");
        UUID branchB = createBranch(fx, "norte");
        createRegister(fx, branchA, token, "register-1", "Front Desk");

        mockMvc.perform(post(url(fx, branchB))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front Desk\"}"))
                .andExpect(status().isCreated());
    }

    @Test
    void createCashRegisterRejectsNonSlugCode() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");

        mockMvc.perform(post(url(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"Register 1\",\"name\":\"Front\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void createCashRegisterWithBranchOfAnotherOrganizationIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenB = adminToken(fxB);
        UUID branchA = createBranch(fxA, "centro");

        mockMvc.perform(post(url(fxB, branchA))
                        .header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void getCashRegisterOfAnotherBranchIs404() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchA = createBranch(fx, "centro");
        UUID branchB = createBranch(fx, "norte");
        String registerId = createRegister(fx, branchA, token, "register-1", "Front");

        mockMvc.perform(get(url(fx, branchB) + "/" + registerId).header("Authorization", bearer(token)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("CASH_REGISTER_NOT_FOUND"));
    }

    @Test
    void listCashRegistersFiltersStatusAndQuery() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        createRegister(fx, branchId, token, "register-1", "Front Desk");
        createRegister(fx, branchId, token, "register-2", "Back Desk");

        mockMvc.perform(get(url(fx, branchId)).header("Authorization", bearer(token)).param("q", "front"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].code").value("register-1"));

        mockMvc.perform(get(url(fx, branchId)).header("Authorization", bearer(token)).param("status", "INACTIVE"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(0));
    }

    @Test
    void patchCashRegisterUpdatesFieldsAndBumpsVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1", "Front");

        mockMvc.perform(patch(url(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Front Desk\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("register-1"))
                .andExpect(jsonPath("$.name").value("Front Desk"))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchCashRegisterIgnoresCodeInBody() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1", "Front");

        mockMvc.perform(patch(url(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"code\":\"hacked\",\"name\":\"Front 2\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("register-1"))
                .andExpect(jsonPath("$.name").value("Front 2"));
    }

    @Test
    void patchCashRegisterStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1", "Front");

        mockMvc.perform(patch(url(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Front 2\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(patch(url(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }
}
