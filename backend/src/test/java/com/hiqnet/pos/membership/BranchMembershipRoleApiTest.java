package com.hiqnet.pos.membership;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipRole;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleRepository;
import com.hiqnet.pos.users.domain.User;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Branch-scoped role assignments of a membership: atomic PUT replacement
 * (duplicates 400, unknown/cross-organization role 404 with rollback),
 * empty-set clearing and the permission guard ({@code branch.membership.read} /
 * {@code branch.membership.roles.manage}, organization scope).
 */
class BranchMembershipRoleApiTest extends AbstractRbacIntegrationTest {

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private RoleRepository roleRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Autowired
    private BranchMembershipRoleRepository membershipRoleRepository;

    @Test
    void getRolesInitiallyEmpty() throws Exception {
        Setup setup = setupMembership("suc-a");
        mockMvc.perform(get(rolesUrl(setup, setup.memberId)).header("Authorization", bearer(setup.token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(0));
    }

    @Test
    void putRolesReplacesTheSetAtomically() throws Exception {
        Setup setup = setupMembership("suc-a");
        UUID roleA = createRole(setup.fx, "role-a", "Role A", "branch.read");
        UUID roleB = createRole(setup.fx, "role-b", "Role B", "branch.read");
        UUID roleC = createRole(setup.fx, "role-c", "Role C", "branch.read");
        assignMembershipRoles(setup.membershipId, roleA, roleB);

        mockMvc.perform(put(rolesUrl(setup, setup.memberId))
                        .header("Authorization", bearer(setup.token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(roleIdsBody(roleB, roleC)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(2))
                .andExpect(jsonPath("$[0].code").value("role-b"))
                .andExpect(jsonPath("$[1].code").value("role-c"));

        assertThat(membershipRolesOf(setup.membershipId)).containsExactlyInAnyOrder(roleB, roleC);
    }

    @Test
    void putRolesRejectsDuplicates() throws Exception {
        Setup setup = setupMembership("suc-a");
        UUID roleA = createRole(setup.fx, "role-a", "Role A", "branch.read");

        mockMvc.perform(put(rolesUrl(setup, setup.memberId))
                        .header("Authorization", bearer(setup.token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(roleIdsBody(roleA, roleA)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void putRolesRejectsUnknownRoleWithoutTouchingExistingSet() throws Exception {
        Setup setup = setupMembership("suc-a");
        UUID roleA = createRole(setup.fx, "role-a", "Role A", "branch.read");
        UUID roleB = createRole(setup.fx, "role-b", "Role B", "branch.read");
        assignMembershipRoles(setup.membershipId, roleA, roleB);

        mockMvc.perform(put(rolesUrl(setup, setup.memberId))
                        .header("Authorization", bearer(setup.token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(roleIdsBody(roleA, UUID.randomUUID(), roleB)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ROLE_NOT_FOUND"));

        // Atomic rollback: the existing set is untouched.
        assertThat(membershipRolesOf(setup.membershipId)).containsExactlyInAnyOrder(roleA, roleB);
    }

    @Test
    void putRolesRejectsCrossOrganizationRole() throws Exception {
        Setup setup = setupMembership("suc-a");
        UUID roleA = createRole(setup.fx, "role-a", "Role A", "branch.read");
        UUID roleInOrgB = createRoleFor(fixture(), "role-b", "Role B");
        assignMembershipRoles(setup.membershipId, roleA);

        mockMvc.perform(put(rolesUrl(setup, setup.memberId))
                        .header("Authorization", bearer(setup.token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(roleIdsBody(roleA, roleInOrgB)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ROLE_NOT_FOUND"));

        // Atomic rollback: no partial mutation happened.
        assertThat(membershipRolesOf(setup.membershipId)).containsExactly(roleA);
    }

    @Test
    void putEmptySetClearsAllRoles() throws Exception {
        Setup setup = setupMembership("suc-a");
        UUID roleA = createRole(setup.fx, "role-a", "Role A", "branch.read");
        assignMembershipRoles(setup.membershipId, roleA);

        mockMvc.perform(put(rolesUrl(setup, setup.memberId))
                        .header("Authorization", bearer(setup.token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(roleIdsBody()))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(0));

        assertThat(membershipRolesOf(setup.membershipId)).isEmpty();
    }

    @Test
    void rolesEndpointsWithoutPermissionAre403() throws Exception {
        Setup setup = setupMembership("suc-a");
        // A DIFFERENT identity: the fixture user holds the all-permissions
        // fixture-admin role created by adminToken(fx), so use a restricted user.
        User restricted = createUserForOrganization(
                setup.fx.orgId().toString(), uniqueEmail("restricted@"), PASSWORD);
        assignRoleToUser(setup.fx, restricted.getId(), "user.read");
        String restrictedToken =
                postLoginExpectingToken(setup.fx.orgCode(), emailFor(restricted), PASSWORD);

        mockMvc.perform(get(rolesUrl(setup, setup.memberId)).header("Authorization", bearer(restrictedToken)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
        mockMvc.perform(put(rolesUrl(setup, setup.memberId))
                        .header("Authorization", bearer(restrictedToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(roleIdsBody()))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void putRolesForUnknownMembershipIs404() throws Exception {
        Setup setup = setupMembership("suc-a");
        UUID roleA = createRole(setup.fx, "role-a", "Role A", "branch.read");

        mockMvc.perform(put(rolesUrl(setup, UUID.randomUUID()))
                        .header("Authorization", bearer(setup.token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(roleIdsBody(roleA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_MEMBERSHIP_NOT_FOUND"));
    }

    // --- fixtures ----------------------------------------------------------

    private record Setup(RbacFixture fx, String token, String branchId, UUID membershipId, UUID memberId) {}

    private Setup setupMembership(String branchCode) throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch =
                branchRepository.saveAndFlush(Branch.create(organization, "Branch " + branchCode, branchCode, null));
        BranchMembership membership = membershipRepository.saveAndFlush(
                BranchMembership.create(fx.orgId(), branch.getId(), fx.userId()));
        return new Setup(fx, token, branch.getId().toString(), membership.getId(), fx.userId());
    }

    private void assignMembershipRoles(UUID membershipId, UUID... roleIds) {
        BranchMembership membership = membershipRepository.findById(membershipId).orElseThrow();
        membershipRoleRepository.saveAllAndFlush(List.of(roleIds).stream()
                .map(id -> BranchMembershipRole.of(membership.getOrganizationId(), membershipId, id))
                .toList());
    }

    private List<UUID> membershipRolesOf(UUID membershipId) {
        BranchMembership membership = membershipRepository.findById(membershipId).orElseThrow();
        return membershipRoleRepository
                .findByOrganizationIdAndMembershipId(membership.getOrganizationId(), membershipId)
                .stream()
                .map(BranchMembershipRole::getRoleId)
                .toList();
    }

    private UUID createRoleFor(RbacFixture fx, String code, String name) {
        Role role = Role.create(fx.orgId(), code, name);
        return roleRepository.saveAndFlush(role).getId();
    }

    /** Creates a role with the given permissions and assigns it to a specific user. */
    private void assignRoleToUser(RbacFixture fx, UUID targetUserId, String... permissions) {
        UUID roleId = createRole(fx, "actor-" + UUID.randomUUID().toString().substring(0, 8), "Actor", permissions);
        userOrganizationRoleRepository.saveAndFlush(
                com.hiqnet.pos.rbac.domain.UserOrganizationRole.of(fx.orgId(), targetUserId, roleId));
    }

    private static String rolesUrl(Setup setup, UUID memberId) {
        return "/api/v1/organizations/" + setup.fx.orgId() + "/branches/" + setup.branchId
                + "/memberships/" + memberId + "/roles";
    }

    private static String roleIdsBody(UUID... roleIds) {
        return List.of(roleIds).stream()
                .map(id -> "\"" + id + "\"")
                .collect(java.util.stream.Collectors.joining(",", "{\"roleIds\":[", "]}"));
    }
}