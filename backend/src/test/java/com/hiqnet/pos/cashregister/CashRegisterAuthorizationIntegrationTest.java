package com.hiqnet.pos.cashregister;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipRole;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import com.hiqnet.pos.organization.domain.Organization;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Cash-register authorization matrix with REAL access JWTs. Cash registers are
 * branch-owned: an org-wide grant reaches every branch; a branch-membership
 * grant reaches ONLY that branch and NEVER another branch of the same
 * organization, nor a branch of another organization.
 */
class CashRegisterAuthorizationIntegrationTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Autowired
    private BranchMembershipRoleRepository membershipRoleRepository;

    @Autowired
    private PlatformTransactionManager transactionManager;

    @Test
    void organizationWideGrantReachesEveryBranch() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        Branch branchB = branchEntity(fx, "suc-b");
        createRoleAndAssign(fx, "register-reader", "Register Reader", "cash-register.read");
        String token = login(fx);

        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        mockMvc.perform(get(url(fx, branchB.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void branchScopedReadAllowsOnlyTheOwnBranch() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        Branch branchB = branchEntity(fx, "suc-b");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "cash-register.read");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
        String token = login(fx); // NO organization-wide role

        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        mockMvc.perform(get(url(fx, branchB.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void branchScopedCreateAndUpdateAreConfinedToTheOwnBranch() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        Branch branchB = branchEntity(fx, "suc-b");
        UUID role = createRole(fx, "centro-manager", "Centro Manager",
                "cash-register.create", "cash-register.update");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
        String token = login(fx);

        String response = mockMvc.perform(post(url(fx, branchA.getId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        String registerId = objectMapper.readTree(response).get("id").asText();

        mockMvc.perform(patch(url(fx, branchA.getId()) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Front 2\"}"))
                .andExpect(status().isOk());

        // The same identity cannot create in another branch of the same organization.
        mockMvc.perform(post(url(fx, branchB.getId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void branchOfAnotherOrganizationIsNotReachable() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        Branch branchA = branchEntity(fxA, "suc-a");
        createRoleAndAssign(fxB, "register-reader", "Register Reader", "cash-register.read");
        String tokenB = login(fxB);

        mockMvc.perform(get(url(fxB, branchA.getId())).header("Authorization", bearer(tokenB)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void branchScopedCashRegisterGrantDoesNotEscalateToOtherDomains() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "cash-register.read");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
        String token = login(fx); // only a branch-scoped cash-register.read grant

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/roles")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/users")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void membershipStatusChangeIsImmediateWithTheSameJwt() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "cash-register.read");
        BranchMembership membership = membershipEntity(fx, branchA.getId());
        assignMembershipRole(fx, membership.getId(), role);
        String token = login(fx);

        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        setMembershipStatus(membership.getId(), BranchMembershipStatus.INACTIVE);
        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        setMembershipStatus(membership.getId(), BranchMembershipStatus.ACTIVE);
        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void branchDeactivationKeepsReadsRecoverableButStillDeniesWrites() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        UUID role = createRole(
                fx, "centro-manager", "Centro Manager", "cash-register.read", "cash-register.create");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
        String token = login(fx);

        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        setBranchStatus(branchA.getId(), BranchStatus.INACTIVE);

        // Reads recover: an already-registered register stays reachable so an
        // open operation can be recovered after the branch is deactivated.
        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        // Writes still require the branch ACTIVE (recovery is read-only).
        mockMvc.perform(post(url(fx, branchA.getId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"new-register\",\"name\":\"New Register\"}"))
                .andExpect(status().isForbidden());

        setBranchStatus(branchA.getId(), BranchStatus.ACTIVE);
        mockMvc.perform(get(url(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    // --- fixtures ----------------------------------------------------------

    private Branch branchEntity(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository.saveAndFlush(Branch.create(organization, "Branch " + code, code, null));
    }

    private BranchMembership membershipEntity(RbacFixture fx, UUID branchId) {
        return membershipRepository.saveAndFlush(BranchMembership.create(fx.orgId(), branchId, fx.userId()));
    }

    private void assignMembershipRole(RbacFixture fx, UUID membershipId, UUID roleId) {
        membershipRoleRepository.saveAndFlush(BranchMembershipRole.of(fx.orgId(), membershipId, roleId));
    }

    private void setMembershipStatus(UUID membershipId, BranchMembershipStatus status) {
        new TransactionTemplate(transactionManager).executeWithoutResult(tx -> {
            BranchMembership fresh = membershipRepository.findById(membershipId).orElseThrow();
            fresh.updateStatus(status);
            membershipRepository.saveAndFlush(fresh);
        });
    }

    private void setBranchStatus(UUID branchId, BranchStatus status) {
        new TransactionTemplate(transactionManager).executeWithoutResult(tx -> {
            Branch fresh = branchRepository.findById(branchId).orElseThrow();
            fresh.updateStatus(status);
            branchRepository.saveAndFlush(fresh);
        });
    }

    private static String url(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }
}
