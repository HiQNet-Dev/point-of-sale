package com.hiqnet.pos.cashshift;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
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
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.rbac.domain.Permission;
import com.hiqnet.pos.rbac.domain.UserOrganizationRole;
import com.hiqnet.pos.users.domain.User;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Acceptance: a branch-scoped operator must recover an already-open shift after
 * the branch is administratively deactivated. The branch must stay discoverable
 * ({@code /auth/me/branches}), the register and shift reads must stay reachable
 * and the shift must be closable — while OPENING a new shift on the deactivated
 * branch is still refused. No endpoint is promoted to organization scope.
 */
class InactiveBranchRecoveryIntegrationTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Autowired
    private BranchMembershipRoleRepository membershipRoleRepository;

    @Autowired
    private PlatformTransactionManager transactionManager;

    @Test
    void branchScopedOperatorRecoversOpenShiftAfterBranchDeactivation() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        String registerId = createRegister(fx, branchA.getId(), "register-1");
        UUID role = createRole(
                fx,
                "centro-ops",
                "Centro Ops",
                "cash-register.read",
                "cash-shift.open",
                "cash-shift.read",
                "cash-shift.close");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
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

        // Administrative disable after the shift is already open.
        setBranchStatus(branchA.getId(), BranchStatus.INACTIVE);

        // 1. The branch stays discoverable to its member.
        mockMvc.perform(get("/api/v1/auth/me/branches").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(branchA.getId().toString()))
                .andExpect(jsonPath("$.items[0].status").value("INACTIVE"));

        // 2. Register reads recover.
        mockMvc.perform(get(registersUrl(fx, branchA.getId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1));

        // 3. Shift reads recover and show the OPEN shift.
        mockMvc.perform(get(listUrl(fx, branchA.getId(), registerId)).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(shiftId))
                .andExpect(jsonPath("$.items[0].status").value("OPEN"));

        // 4. The shift is closable.
        mockMvc.perform(post(closeUrl(fx, branchA.getId(), registerId, shiftId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":10.0000}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("CLOSED"));
    }

    @Test
    void openingOnDeactivatedBranchIsStillRefused() throws Exception {
        RbacFixture fx = fixture();
        Branch branchA = branchEntity(fx, "suc-a");
        String registerId = createRegister(fx, branchA.getId(), "register-1");
        UUID role = createRole(
                fx,
                "centro-ops",
                "Centro Ops",
                "cash-register.read",
                "cash-shift.open",
                "cash-shift.read",
                "cash-shift.close");
        assignMembershipRole(fx, membershipEntity(fx, branchA.getId()).getId(), role);
        String token = login(fx);

        setBranchStatus(branchA.getId(), BranchStatus.INACTIVE);

        // Opening requires the branch ACTIVE: the recovery relaxation never
        // applies to writes.
        mockMvc.perform(post(openUrl(fx, branchA.getId(), registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":10.0000}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
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

    private void setBranchStatus(UUID branchId, BranchStatus status) {
        new TransactionTemplate(transactionManager).executeWithoutResult(tx -> {
            Branch fresh = branchRepository.findById(branchId).orElseThrow();
            fresh.updateStatus(status);
            branchRepository.saveAndFlush(fresh);
        });
    }

    /** A separate org-wide admin (NOT the fixture user) so fixture grants stay restricted. */
    private String createRegister(RbacFixture fx, UUID branchId, String code) throws Exception {
        User owner = createUserForOrganization(fx.orgId().toString(), uniqueEmail("owner@"), PASSWORD);
        List<String> all = permissionRepository.findAllByOrderByCodeAsc().stream()
                .map(Permission::getCode)
                .toList();
        UUID roleId = createRole(
                fx,
                "owner" + UUID.randomUUID().toString().substring(0, 8),
                "Owner Admin",
                all.toArray(String[]::new));
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
