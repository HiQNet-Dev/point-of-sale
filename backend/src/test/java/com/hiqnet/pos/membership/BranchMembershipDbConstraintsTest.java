package com.hiqnet.pos.membership;

import static org.assertj.core.api.Assertions.assertThatThrownBy;

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
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * PostgreSQL is the FINAL barrier (not Java): the composite foreign keys must
 * reject cross-organization memberships and membership roles even when the
 * entity is inserted directly, bypassing the service validation. Each case
 * runs in its own transaction so the failed insert rolls back.
 */
class BranchMembershipDbConstraintsTest extends AbstractRbacIntegrationTest {

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

    @Autowired
    private PlatformTransactionManager transactionManager;

    private String branchId;
    private java.util.UUID userA;
    private java.util.UUID userB;
    private java.util.UUID orgA;

    @Test
    void membershipWithUserOfAnotherOrganizationIsRejected() {
        orgA = fixture().orgId();
        RbacFixture fxB = fixture();
        userA = fixtureUser();
        userB = fxB.userId();
        branchId = createBranch(orgA).toString();

        assertThatThrownBy(() -> insertMembership(orgA, java.util.UUID.fromString(branchId), userB))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void membershipWithBranchOfAnotherOrganizationIsRejected() {
        orgA = fixture().orgId();
        RbacFixture fxB = fixture();
        userA = fixtureUser();
        userB = fxB.userId();
        String branchInOrgB = createBranch(fxB.orgId()).toString();

        assertThatThrownBy(() -> insertMembership(orgA, java.util.UUID.fromString(branchInOrgB), userA))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void membershipRoleOfAnotherOrganizationIsRejected() {
        orgA = fixture().orgId();
        RbacFixture fxB = fixture();
        userA = fixtureUser();
        java.util.UUID roleInOrgB = createRole(fxB.orgId(), "role-b");
        java.util.UUID membershipId = insertMembership(orgA, java.util.UUID.fromString(createBranch(orgA).toString()), userA);

        assertThatThrownBy(() -> insertMembershipRole(orgA, membershipId, roleInOrgB))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void duplicateMembershipIsRejectedByTheUniqueConstraint() {
        orgA = fixture().orgId();
        userA = fixtureUser();
        java.util.UUID branch = java.util.UUID.fromString(createBranch(orgA).toString());

        insertMembership(orgA, branch, userA);
        assertThatThrownBy(() -> insertMembership(orgA, branch, userA))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    // --- helpers -----------------------------------------------------------

    private java.util.UUID fixtureUser() {
        User user = User.create(orgA, uniqueEmail("db-u@"), passwordEncoder.encode(PASSWORD));
        return userRepository.saveAndFlush(user).getId();
    }

    private java.util.UUID createBranch(java.util.UUID organizationId) {
        Organization organization = organizationRepository.findById(organizationId).orElseThrow();
        Branch branch = Branch.create(organization, "Branch db", "db-" + java.util.UUID.randomUUID().toString().substring(0, 8), null);
        return branchRepository.saveAndFlush(branch).getId();
    }

    private java.util.UUID createRole(java.util.UUID organizationId, String code) {
        Role role = Role.create(organizationId, code, "Role " + code);
        return roleRepository.saveAndFlush(role).getId();
    }

    private java.util.UUID insertMembership(java.util.UUID organizationId, java.util.UUID branch, java.util.UUID user) {
        BranchMembership[] result = new BranchMembership[1];
        new TransactionTemplate(transactionManager).executeWithoutResult(status ->
                result[0] = membershipRepository.saveAndFlush(
                        BranchMembership.create(organizationId, branch, user)));
        return result[0].getId();
    }

    private void insertMembershipRole(java.util.UUID organizationId, java.util.UUID membershipId, java.util.UUID roleId) {
        new TransactionTemplate(transactionManager).executeWithoutResult(status ->
                membershipRoleRepository.saveAndFlush(
                        BranchMembershipRole.of(organizationId, membershipId, roleId)));
    }
}