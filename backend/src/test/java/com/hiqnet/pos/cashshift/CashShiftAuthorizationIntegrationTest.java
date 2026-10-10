package com.hiqnet.pos.cashshift;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
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
import com.hiqnet.pos.rbac.domain.Permission;
import com.hiqnet.pos.rbac.domain.UserOrganizationRole;
import com.hiqnet.pos.users.domain.User;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Cash shifts are branch-owned and branch-scoped in authorization: an org-wide
 * grant reaches every branch; a branch-membership grant reaches ONLY that branch
 * and never another branch of the same org, nor a branch of another org.
 */
class CashShiftAuthorizationIntegrationTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Autowired
    private BranchMembershipRoleRepository membershipRoleRepository;

    @Test
    void orgWideGrantReachesEveryBranch() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        Branch branchB = branchEntity(fx, "suc-b");
        String registerA = createRegister(fx, branchA.getId(), "r1");
        String registerB = createRegister(fx, branchB.getId(), "r1");
        createRoleAndAssign(fx, "shift-reader", "Shift Reader", "cash-shift.read");
        String token = login(fx);

        mockMvc.perform(get(listUrl(fx, branchA.getId(), registerA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        mockMvc.perform(get(listUrl(fx, branchB.getId(), registerB)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void branchScopedOpenAndReadAreConfinedToTheOwnBranch() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        Branch branchB = branchEntity(fx, "suc-b");
        String registerA = createRegister(fx, branchA.getId(), "register-1");
        String registerB = createRegister(fx, branchB.getId(), "register-1");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "cash-shift.read", "cash-shift.open");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
        String token = login(fx); // NO organization-wide role

        mockMvc.perform(post(openUrl(fx, branchA.getId(), registerA))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":10.0000}"))
                .andExpect(status().isCreated());

        mockMvc.perform(post(openUrl(fx, branchB.getId(), registerB))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":10.0000}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));

        mockMvc.perform(get(listUrl(fx, branchB.getId(), registerB)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void closeRequiresClosePermissionNotOpen() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        String registerId = createRegister(fx, branchA.getId(), "register-1");
        UUID openRole = createRole(fx, "opener", "Opener", "cash-shift.open");
        UUID readRole = createRole(fx, "reader", "Reader", "cash-shift.read");
        UUID closeRole = createRole(fx, "closer", "Closer", "cash-shift.close");
        UUID membershipId = membershipEntity(fx, branchA.getId()).getId();
        assignMembershipRole(fx, membershipId, openRole);
        String token = login(fx);

        String response = mockMvc.perform(post(openUrl(fx, branchA.getId(), registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":10.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        String shiftId = objectMapper.readTree(response).get("id").asText();

        // open permission alone cannot close
        mockMvc.perform(post(closeUrl(fx, branchA.getId(), registerId, shiftId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":10.0000}"))
                .andExpect(status().isForbidden());

        // read permission alone cannot close
        assignMembershipRole(fx, membershipId, readRole);
        mockMvc.perform(post(closeUrl(fx, branchA.getId(), registerId, shiftId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":10.0000}"))
                .andExpect(status().isForbidden());

        // close permission alone can close, even without open
        assignMembershipRole(fx, membershipId, closeRole);
        mockMvc.perform(post(closeUrl(fx, branchA.getId(), registerId, shiftId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":10.0000}"))
                .andExpect(status().isOk());
    }

    @Test
    void branchOfAnotherOrganizationIsNotReachable() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        Branch branchA = branchEntity(fxA, "suc-a");
        createRoleAndAssign(fxB, "shift-reader", "Shift Reader", "cash-shift.read", "cash-shift.open");
        String tokenB = login(fxB);

        mockMvc.perform(get(listUrl(fxB, branchA.getId(), UUID.randomUUID().toString()))
                        .header("Authorization", bearer(tokenB)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void branchScopedShiftGrantDoesNotEscalateToOtherDomains() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "cash-shift.read");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
        String token = login(fx);

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/cash-registers")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
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

    /** A separate org-wide admin (NOT the fixture user) so fixture grants stay restricted. */
    private String createRegister(RbacFixture fx, UUID branchId, String code) throws Exception {
        User owner = createUserForOrganization(fx.orgId().toString(), uniqueEmail("owner@"), PASSWORD);
        List<String> all = permissionRepository.findAllByOrderByCodeAsc().stream()
                .map(Permission::getCode)
                .toList();
        UUID roleId = createRole(
                fx, "owner" + UUID.randomUUID().toString().substring(0, 8), "Owner Admin", all.toArray(String[]::new));
        userOrganizationRoleRepository.saveAndFlush(UserOrganizationRole.of(fx.orgId(), owner.getId(), roleId));
        String ownerToken = postLoginExpectingToken(fx.orgCode(), owner.getEmail(), PASSWORD);

        String response = mockMvc.perform(post(registersUrl(fx, branchId))
                        .header("Authorization", bearer(ownerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + code + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private static String registersUrl(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }

    private static String listUrl(RbacFixture fx, UUID branchId, String registerId) {
        return registersUrl(fx, branchId) + "/" + registerId + "/cash-shifts";
    }

    private static String openUrl(RbacFixture fx, UUID branchId, String registerId) {
        return listUrl(fx, branchId, registerId);
    }

    private static String closeUrl(RbacFixture fx, UUID branchId, String registerId, String shiftId) {
        return listUrl(fx, branchId, registerId) + "/" + shiftId + "/close";
    }
}
