package com.hiqnet.pos.membership;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryRepository;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipRole;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleId;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleRepository;
import com.hiqnet.pos.rbac.domain.RoleStatus;
import com.hiqnet.pos.users.domain.User;
import java.math.BigDecimal;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.security.oauth2.jwt.JwtDecoder;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Branch-scoped authorization matrix with REAL access JWTs. The SAME JWT must
 * reflect membership/role/branch/user/organization changes immediately because
 * authorization is PostgreSQL-authoritative (JWT identity-only). Two scopes are
 * proven: an org-wide grant reaches every branch; a branch-membership grant
 * reaches ONLY that branch and NEVER escalates to the organization-wide
 * endpoints.
 */
class BranchAccessAuthorizationIntegrationTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private RoleRepository roleRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Autowired
    private BranchMembershipRoleRepository membershipRoleRepository;

    @Autowired
    private CategoryRepository categoryRepository;

    @Autowired
    private ProductRepository productRepository;

    @Autowired
    private ModifierGroupRepository modifierGroupRepository;

    @Autowired
    private PlatformTransactionManager transactionManager;

    @Autowired
    private JwtDecoder jwtDecoder;

    @Test
    void organizationWideBranchReadGrantsAccessToEveryBranch() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        String branchB = createBranch(fx, "suc-b");
        createRoleAndAssign(fx, "branch-reader", "Branch Reader", "branch.read");
        String token = login(fx);

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        mockMvc.perform(get(branchUrl(fx, branchB)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void branchScopedReadAllowsOnlyTheOwnBranch() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        String branchB = createBranch(fx, "suc-b");
        UUID roleA = createRole(fx, "centro-manager", "Centro Manager", "branch.read");
        UUID membershipA = createMembership(fx, UUID.fromString(branchA));
        assignMembershipRole(fx, membershipA, roleA);
        String token = login(fx); // NO organization-wide role

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        mockMvc.perform(get(branchUrl(fx, branchB)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void branchScopedRoleNeverEscalatesToOrganizationWideEndpoints() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        UUID targetUser = createTargetUser(fx, "target@");
        // The branch-scoped role contains organization-scope permissions: they
        // must NOT authorize the organization-wide endpoints (anti-escalation).
        UUID role = createRole(
                fx,
                "branch-lead",
                "Branch Lead",
                "user.read",
                "role.assign",
                "organization.update",
                "branch.read",
                "branch.membership.roles.manage");
        UUID membershipA = createMembership(fx, UUID.fromString(branchA));
        assignMembershipRole(fx, membershipA, role);
        String token = login(fx);

        // Branch scope works for the own branch...
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        // ...but NEVER for organization-scope endpoints, even though the role
        // literally contains those permissions.
        mockMvc.perform(get(usersUrl(fx)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
        mockMvc.perform(get(orgUrl(fx)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(patch(orgUrl(fx))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isForbidden());
        mockMvc.perform(put("/api/v1/organizations/" + fx.orgId() + "/users/" + targetUser + "/roles")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[]}"))
                .andExpect(status().isForbidden());
        // Membership management requires organization scope too.
        mockMvc.perform(put("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchA
                                + "/memberships/" + targetUser + "/roles")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[]}"))
                .andExpect(status().isForbidden());
        // Organization-scope collection listing stays closed.
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void branchScopedRoleNeverEscalatesToOrganizationWideCatalog() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        UUID categoryId = categoryEntity(fx, "bebidas").getId();
        UUID productId = productEntity(fx, "AGUA").getId();
        // The branch-scoped role literally contains the organization-scope
        // catalog permissions: they must NOT authorize the organization-wide
        // catalog PATCH endpoints. This locks hasPermission(...) against an
        // accidental refactor to hasBranchPermission(...) for the catalog.
        UUID role = createRole(
                fx,
                "catalog-lead",
                "Catalog Lead",
                "branch.read",
                "category.update",
                "product.update");
        UUID membershipA = createMembership(fx, UUID.fromString(branchA));
        assignMembershipRole(fx, membershipA, role);
        String token = login(fx); // NO organization-wide role

        // Branch scope works for the own branch...
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        // ...but the branch-scoped catalog permissions NEVER reach the
        // organization-wide catalog endpoints, even for an existing resource.
        mockMvc.perform(patch(categoryUrl(fx, categoryId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
        mockMvc.perform(patch(productUrl(fx, productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void branchScopedRoleNeverEscalatesToOrganizationWideModifiers() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        UUID groupId = modifierGroupEntity(fx, "milk").getId();
        // The branch-scoped role literally contains organization-scope modifier
        // permissions: they must NOT authorize the organization-wide endpoints.
        UUID role = createRole(
                fx,
                "modifier-lead",
                "Modifier Lead",
                "branch.read",
                "modifier.group.update",
                "product.modifiers.manage");
        UUID membershipA = createMembership(fx, UUID.fromString(branchA));
        assignMembershipRole(fx, membershipA, role);
        String token = login(fx); // NO organization-wide role

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void membershipStatusChangeIsImmediateWithTheSameJwt() throws Exception {        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "branch.read");
        BranchMembership membership = membershipEntity(fx, branchA);
        assignMembershipRole(fx, membership.getId(), role);
        String token = login(fx);

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        setMembershipStatus(membership.getId(), BranchMembershipStatus.INACTIVE);
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        setMembershipStatus(membership.getId(), BranchMembershipStatus.ACTIVE);
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void membershipRoleRemovalAndReassignmentAreImmediate() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "branch.read");
        UUID membership = createMembership(fx, UUID.fromString(branchA));
        assignMembershipRole(fx, membership, role);
        String token = login(fx);

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        removeMembershipRole(fx.orgId(), membership, role);
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        assignMembershipRole(fx, membership, role);
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void branchRoleDeactivationIsImmediateWithTheSameJwt() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        Role role = roleEntity(fx, "centro-manager", "Centro Manager", "branch.read");
        UUID membership = createMembership(fx, UUID.fromString(branchA));
        assignMembershipRole(fx, membership, role.getId());
        String token = login(fx);

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        setRoleStatus(role.getId(), RoleStatus.INACTIVE);
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        setRoleStatus(role.getId(), RoleStatus.ACTIVE);
        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void branchDeactivationDeniesBranchScopedPermissionImmediately() throws Exception {
        RbacFixture fx = fixture();
        Branch branch = branchEntity(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "branch.read");
        UUID membership = createMembership(fx, branch.getId());
        assignMembershipRole(fx, membership, role);
        String token = login(fx);

        mockMvc.perform(get(branchUrl(fx, branch.getId().toString())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        setBranchStatus(branch.getId(), BranchStatus.INACTIVE);
        mockMvc.perform(get(branchUrl(fx, branch.getId().toString())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        setBranchStatus(branch.getId(), BranchStatus.ACTIVE);
        mockMvc.perform(get(branchUrl(fx, branch.getId().toString())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void inactiveUserDeniesBranchPermissionEvenWithActiveMembership() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "branch.read");
        createMembershipAndAssign(fx, branchA, role);
        // Obtain a REAL access token while the identity is ACTIVE, then
        // deactivate the user: the same JWT must stop granting branch access.
        String token = login(fx);
        User actor = userRepository.findById(fx.userId()).orElseThrow();
        actor.deactivate();
        userRepository.saveAndFlush(actor);

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void inactiveOrganizationDeniesBranchPermissionEvenWithActiveMembership() throws Exception {
        RbacFixture fx = fixture();
        String branchA = createBranch(fx, "suc-a");
        UUID role = createRole(fx, "centro-manager", "Centro Manager", "branch.read");
        createMembershipAndAssign(fx, branchA, role);
        String token = login(fx);

        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        organization.updateStatus(OrganizationStatus.INACTIVE);
        organizationRepository.saveAndFlush(organization);

        mockMvc.perform(get(branchUrl(fx, branchA)).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void accessJwtHasNoBranchOrMembershipClaims() throws Exception {
        RbacFixture fx = fixture();
        createRoleAndAssign(fx, "branch-reader", "Branch Reader", "branch.read");
        String token = login(fx);

        Jwt jwt = jwtDecoder.decode(token);
        assertThat(jwt.getClaimAsStringList("roles")).isNull();
        assertThat(jwt.getClaimAsStringList("permissions")).isNull();
        assertThat(jwt.getClaimAsStringList("branches")).isNull();
        assertThat(jwt.getClaimAsStringList("branchIds")).isNull();
        assertThat(jwt.getClaimAsStringList("memberships")).isNull();
        assertThat(jwt.getSubject()).isEqualTo(fx.userId().toString());
        assertThat(jwt.getClaimAsString("org_id")).isEqualTo(fx.orgId().toString());
    }

    // --- fixtures ----------------------------------------------------------

    private String createBranch(RbacFixture fx, String code) {
        return branchEntity(fx, code).getId().toString();
    }

    private Category categoryEntity(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return categoryRepository.saveAndFlush(Category.create(organization, code, "Category " + code));
    }

    private Product productEntity(RbacFixture fx, String sku) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return productRepository.saveAndFlush(
                Product.create(organization, sku, "Product " + sku, new BigDecimal("5.0000")));
    }

    private ModifierGroup modifierGroupEntity(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return modifierGroupRepository.saveAndFlush(
                ModifierGroup.create(organization, code, "Group " + code, 0, 1));
    }

    private Branch branchEntity(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository.saveAndFlush(Branch.create(organization, "Branch " + code, code, null));
    }

    private BranchMembership membershipEntity(RbacFixture fx, String branchId) {
        return membershipRepository.saveAndFlush(
                BranchMembership.create(fx.orgId(), UUID.fromString(branchId), fx.userId()));
    }

    private UUID createMembership(RbacFixture fx, UUID branchId) {
        return membershipEntity(fx, branchId.toString()).getId();
    }

    private void createMembershipAndAssign(RbacFixture fx, String branchId, UUID roleId) {
        UUID membership = createMembership(fx, UUID.fromString(branchId));
        assignMembershipRole(fx, membership, roleId);
    }

    private void assignMembershipRole(RbacFixture fx, UUID membershipId, UUID roleId) {
        membershipRoleRepository.saveAndFlush(
                BranchMembershipRole.of(fx.orgId(), membershipId, roleId));
    }

    private void removeMembershipRole(UUID organizationId, UUID membershipId, UUID roleId) {
        new TransactionTemplate(transactionManager).executeWithoutResult(status ->
                membershipRoleRepository.deleteById(new BranchMembershipRoleId(organizationId, membershipId, roleId)));
    }

    private void setMembershipStatus(UUID membershipId, BranchMembershipStatus status) {
        BranchMembership membership = membershipRepository.findById(membershipId).orElseThrow();
        membership.updateStatus(status);
        membershipRepository.saveAndFlush(membership);
    }

    private void setBranchStatus(UUID branchId, BranchStatus status) {
        new TransactionTemplate(transactionManager).executeWithoutResult(tx -> {
            // Fresh managed load: never mutate an entity detached across HTTP calls.
            Branch fresh = branchRepository.findById(branchId).orElseThrow();
            fresh.updateStatus(status);
            branchRepository.saveAndFlush(fresh);
        });
    }

    private Role roleEntity(RbacFixture fx, String code, String name, String... permissions) {
        Role role = Role.create(fx.orgId(), code, name);
        roleRepository.saveAndFlush(role);
        grantPermissionsToRole(role.getId(), permissions);
        return roleRepository.findById(role.getId()).orElseThrow();
    }

    private void setRoleStatus(UUID roleId, RoleStatus status) {
        Role role = roleRepository.findById(roleId).orElseThrow();
        role.updateStatus(status);
        roleRepository.saveAndFlush(role);
    }

    private UUID createTargetUser(RbacFixture fx, String emailPrefix) {
        User user = User.create(fx.orgId(), uniqueEmail(emailPrefix), passwordEncoder.encode(PASSWORD));
        userRepository.saveAndFlush(user);
        return user.getId();
    }

    private static String branchUrl(RbacFixture fx, String branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId;
    }

    private static String categoryUrl(RbacFixture fx, UUID categoryId) {
        return "/api/v1/organizations/" + fx.orgId() + "/categories/" + categoryId;
    }

    private static String productUrl(RbacFixture fx, UUID productId) {
        return "/api/v1/organizations/" + fx.orgId() + "/products/" + productId;
    }

    private static String usersUrl(RbacFixture fx) {
        return "/api/v1/organizations/" + fx.orgId() + "/users";
    }

    private static String orgUrl(RbacFixture fx) {
        return "/api/v1/organizations/" + fx.orgId();
    }
}