package com.hiqnet.pos.membership;

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
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
import com.hiqnet.pos.users.domain.User;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Administrative API of branch memberships under the REAL security chain: the
 * requester holds {@code branch.membership.*} permissions org-wide (never a
 * branch-scoped grant). Verifies creation guards (org/branch/user ACTIVE,
 * same-organization, duplicate), pagination, the expectedVersion contract and
 * the 403/404 cross-organization split.
 */
class BranchMembershipApiTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Test
    void createMembershipReturns201AndActive() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");
        User member = createTargetUser(fx, "member-a@");

        mockMvc.perform(post(membershipsUrl(fx.orgId(), branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + member.getId() + "\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.branchId").value(branchId))
                .andExpect(jsonPath("$.userId").value(member.getId().toString()))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void createMembershipForInactiveUserIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");
        User member = createTargetUser(fx, "member-i@");
        member.deactivate();
        userRepository.saveAndFlush(member);

        mockMvc.perform(post(membershipsUrl(fx.orgId(), branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + member.getId() + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("USER_INACTIVE"));
    }

    @Test
    void createMembershipForInactiveBranchIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Branch branch = branchOf(fx, "suc-b");
        branch.updateStatus(BranchStatus.INACTIVE);
        branchRepository.saveAndFlush(branch);
        User member = createTargetUser(fx, "member-b@");

        mockMvc.perform(post(membershipsUrl(fx.orgId(), branch.getId().toString()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + member.getId() + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BRANCH_INACTIVE"));
    }

    @Test
    void createMembershipForInactiveOrganizationIs403() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");
        User member = createTargetUser(fx, "member-o@");

        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        organization.updateStatus(OrganizationStatus.INACTIVE);
        organizationRepository.saveAndFlush(organization);

        // With the organization INACTIVE the database-authoritative check denies
        // BEFORE the service is reached: the same JWT stops granting any
        // permission in that organization (403). The service-level 409
        // ORGANIZATION_INACTIVE exists but is unreachable via HTTP for a member
        // of the deactivated organization.
        mockMvc.perform(post(membershipsUrl(fx.orgId(), branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + member.getId() + "\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createMembershipForBranchOfAnotherOrganizationIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String branchB = createBranch(fxB, "suc-b1");
        User memberA = createTargetUser(fxA, "member-x@");

        mockMvc.perform(post(membershipsUrl(fxA.orgId(), branchB))
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + memberA.getId() + "\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void createMembershipForUnknownUserIs404() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");

        mockMvc.perform(post(membershipsUrl(fx.orgId(), branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + UUID.randomUUID() + "\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void createDuplicateMembershipIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");
        User member = createTargetUser(fx, "member-d@");
        String body = "{\"userId\":\"" + member.getId() + "\"}";

        mockMvc.perform(post(membershipsUrl(fx.orgId(), branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isCreated());

        mockMvc.perform(post(membershipsUrl(fx.orgId(), branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BRANCH_MEMBERSHIP_ALREADY_EXISTS"));
    }

    @Test
    void listMembershipsIsPagedAndFiltersByStatus() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");
        UUID m1 = createMembership(fx, branchId, "member-l1@");
        UUID m2 = createMembership(fx, branchId, "member-l2@");
        UUID m3 = createMembership(fx, branchId, "member-l3@");
        mockMvc.perform(patch(membershipsUrl(fx.orgId(), branchId) + "/" + m3)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(get(membershipsUrl(fx.orgId(), branchId)).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(3))
                .andExpect(jsonPath("$.items.length()").value(3));

        mockMvc.perform(get(membershipsUrl(fx.orgId(), branchId))
                        .param("status", "INACTIVE")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].userId").value(m3.toString()));

        mockMvc.perform(get(membershipsUrl(fx.orgId(), branchId))
                        .param("size", "1")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalPages").value(3))
                .andExpect(jsonPath("$.items.length()").value(1));
    }

    @Test
    void getMembershipOfUserIs200() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");
        UUID member = createMembership(fx, branchId, "member-g@");

        mockMvc.perform(get(membershipsUrl(fx.orgId(), branchId) + "/" + member)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.userId").value(member.toString()))
                .andExpect(jsonPath("$.status").value("ACTIVE"));
    }

    @Test
    void getUnknownMembershipIs404() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");

        mockMvc.perform(get(membershipsUrl(fx.orgId(), branchId) + "/" + UUID.randomUUID())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_MEMBERSHIP_NOT_FOUND"));
    }

    @Test
    void patchMembershipHonoursExpectedVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranch(fx, "suc-a");
        UUID member = createMembership(fx, branchId, "member-p@");

        mockMvc.perform(patch(membershipsUrl(fx.orgId(), branchId) + "/" + member)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(1));

        // Stale write against the same version must be rejected.
        mockMvc.perform(patch(membershipsUrl(fx.orgId(), branchId) + "/" + member)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));

        // Current version still works.
        mockMvc.perform(patch(membershipsUrl(fx.orgId(), branchId) + "/" + member)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"ACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(2));
    }

    @Test
    void membershipApiWithoutPermissionIs403() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx); // valid JWT, no roles
        String branchId = createBranch(fx, "suc-a");

        mockMvc.perform(post(membershipsUrl(fx.orgId(), branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + fx.userId() + "\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));

        mockMvc.perform(get(membershipsUrl(fx.orgId(), branchId)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void crossOrganizationBoundaryIs403() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA); // all permissions in A, none in B
        String branchB = createBranch(fxB, "suc-b1");

        mockMvc.perform(get(membershipsUrl(fxB.orgId(), branchB)).header("Authorization", bearer(tokenA)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void foreignBranchLookupUnderOwnOrganizationIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA); // authorization exists in A
        String branchB = createBranch(fxB, "suc-b1");

        mockMvc.perform(get(membershipsUrl(fxA.orgId(), branchB)).header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    // --- fixtures ----------------------------------------------------------

    private String createBranch(RbacFixture fx, String code) {
        return branchOf(fx, code).getId().toString();
    }

    private Branch branchOf(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch = Branch.create(organization, "Branch " + code, code, null);
        return branchRepository.saveAndFlush(branch);
    }

    private User createTargetUser(RbacFixture fx, String prefix) {
        User user = User.create(fx.orgId(), uniqueEmail(prefix), passwordEncoder.encode(PASSWORD));
        return userRepository.saveAndFlush(user);
    }

    private UUID createMembership(RbacFixture fx, String branchId, String emailPrefix) {
        User member = createTargetUser(fx, emailPrefix);
        membershipRepository.saveAndFlush(
                BranchMembership.create(fx.orgId(), UUID.fromString(branchId), member.getId()));
        // The membership endpoints address the membership by USER id (one
        // membership per user+branch), so the helper returns the user id.
        return member.getId();
    }

    private static String membershipsUrl(UUID organizationId, String branchId) {
        return "/api/v1/organizations/" + organizationId + "/branches/" + branchId + "/memberships";
    }
}