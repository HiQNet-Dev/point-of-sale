package com.hiqnet.pos.auth;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
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
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Self operational context: {@code /auth/me} carries the organization context and
 * {@code /auth/me/branches} exposes the branches the identity may see. The list
 * is identity-derived (no organization/user in the path) and its visibility rule
 * is deliberately NOT authorization: membership-branch users see only their own
 * branches, org-wide {@code branch.read} users see every branch of their
 * organization, a deactivated branch stays discoverable to its members, and the
 * organization-wide branch listing stays closed to branch-scoped users.
 */
class SelfOperationalContextApiTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Autowired
    private BranchMembershipRoleRepository membershipRoleRepository;

    @Autowired
    private PlatformTransactionManager transactionManager;

    @Test
    void meIncludesOrganizationOperationalContext() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.userId").value(fx.userId().toString()))
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.organization.currency").value("MXN"))
                .andExpect(jsonPath("$.organization.locale").value("es-MX"))
                .andExpect(jsonPath("$.organization.timezone").value("America/Mexico_City"));
    }

    @Test
    void myBranchesWithoutOrganizationWideReadReturnsOnlyMembershipBranches() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        branchEntity(fx, "suc-b");
        UUID role = createRole(fx, "branch-reader", "Branch Reader", "branch.read");
        assignMembershipRole(fx, createMembership(fx, branchA.getId()), role);
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(branchA.getId().toString()));
    }

    @Test
    void organizationWideBranchReadReturnsEveryBranchOfTheOrganization() throws Exception {
        RbacFixture fx = fixture();
        branchEntity(fx, "suc-a");
        branchEntity(fx, "suc-b");
        createRoleAndAssign(fx, "branch-reader", "Branch Reader", "branch.read");
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(2));
    }

    @Test
    void inactiveBranchRemainsDiscoverableToItsMembers() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        UUID role = createRole(fx, "branch-reader", "Branch Reader", "branch.read");
        assignMembershipRole(fx, createMembership(fx, branchA.getId()), role);
        setBranchStatus(branchA.getId(), BranchStatus.INACTIVE);
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(branchA.getId().toString()))
                .andExpect(jsonPath("$.items[0].status").value("INACTIVE"));
    }

    @Test
    void inactiveMembershipIsNotVisible() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        UUID membership = createMembership(fx, branchA.getId());
        setMembershipStatus(membership, BranchMembershipStatus.INACTIVE);
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(0));
    }

    @Test
    void paginationIsApplied() throws Exception {
        RbacFixture fx = fixture();
        for (int i = 1; i <= 3; i++) {
            Branch branch = branchEntity(fx, "suc-" + i);
            createMembership(fx, branch.getId());
        }
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me/branches?size=2").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(3))
                .andExpect(jsonPath("$.totalPages").value(2))
                .andExpect(jsonPath("$.size").value(2))
                .andExpect(jsonPath("$.items.length()").value(2));

        mockMvc.perform(get("/api/v1/auth/me/branches?size=2&page=1").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.page").value(1))
                .andExpect(jsonPath("$.items.length()").value(1));
    }

    @Test
    void filtersByStatusAndQuery() throws Exception {
        RbacFixture fx = fixture();
        Branch alfa = branchEntity(fx, "alfa");
        Branch beta = branchEntity(fx, "beta");
        createMembership(fx, alfa.getId());
        createMembership(fx, beta.getId());
        setBranchStatus(beta.getId(), BranchStatus.INACTIVE);
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me/branches?status=INACTIVE").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(beta.getId().toString()));

        mockMvc.perform(get("/api/v1/auth/me/branches?q=alfa").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(alfa.getId().toString()));
    }

    @Test
    void crossOrganizationBranchesAreNeverVisible() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        Branch branchA = branchEntity(fxA, "suc-a");
        branchEntity(fxB, "suc-b");
        UUID roleA = createRole(fxA, "branch-reader", "Branch Reader", "branch.read");
        assignMembershipRole(fxA, createMembership(fxA, branchA.getId()), roleA);
        String tokenA = login(fxA);

        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(tokenA)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(branchA.getId().toString()));
    }

    @Test
    void emptyPageWhenNoVisibleBranch() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);

        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(0))
                .andExpect(jsonPath("$.totalPages").value(0))
                .andExpect(jsonPath("$.items.length()").value(0));
    }

    @Test
    void branchScopedMemberStillCannotListOrganizationBranches() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        UUID role = createRole(fx, "branch-reader", "Branch Reader", "branch.read");
        assignMembershipRole(fx, createMembership(fx, branchA.getId()), role);
        String token = login(fx);

        // Self-scoped discovery works...
        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1));

        // ...but the organization-scoped listing stays closed (no escalation).
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void requiresAuthentication() throws Exception {
        mockMvc.perform(get("/api/v1/auth/me/branches")).andExpect(status().isUnauthorized());
    }

    // --- fixtures ----------------------------------------------------------

    private Branch branchEntity(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository.saveAndFlush(Branch.create(organization, "Branch " + code, code, null));
    }

    private UUID createMembership(RbacFixture fx, UUID branchId) {
        return membershipRepository
                .saveAndFlush(BranchMembership.create(fx.orgId(), branchId, fx.userId()))
                .getId();
    }

    private void assignMembershipRole(RbacFixture fx, UUID membershipId, UUID roleId) {
        membershipRoleRepository.saveAndFlush(BranchMembershipRole.of(fx.orgId(), membershipId, roleId));
    }

    private void setMembershipStatus(UUID membershipId, BranchMembershipStatus status) {
        BranchMembership membership = membershipRepository.findById(membershipId).orElseThrow();
        membership.updateStatus(status);
        membershipRepository.saveAndFlush(membership);
    }

    private void setBranchStatus(UUID branchId, BranchStatus status) {
        new TransactionTemplate(transactionManager).executeWithoutResult(tx -> {
            Branch fresh = branchRepository.findById(branchId).orElseThrow();
            fresh.updateStatus(status);
            branchRepository.saveAndFlush(fresh);
        });
    }
}
