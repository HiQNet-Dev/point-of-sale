package com.hiqnet.pos;

import com.hiqnet.pos.auth.AbstractAuthIntegrationTest;
import com.hiqnet.pos.rbac.domain.Permission;
import com.hiqnet.pos.rbac.domain.PermissionRepository;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RolePermission;
import com.hiqnet.pos.rbac.domain.RolePermissionRepository;
import com.hiqnet.pos.rbac.domain.RoleRepository;
import com.hiqnet.pos.rbac.domain.UserOrganizationRole;
import com.hiqnet.pos.rbac.domain.UserOrganizationRoleRepository;
import com.hiqnet.pos.users.domain.User;
import java.util.Arrays;
import java.util.List;
import java.util.UUID;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Base for ADMINISTRATIVE API tests under the REAL security chain: requests
 * carry a real access JWT and authorization is database-authoritative (roles +
 * permissions seeded through the rbac repositories). Everything an RBAC test
 * asserts about HTTP is therefore truthful (no test-only bypass).
 */
public abstract class AbstractRbacIntegrationTest extends AbstractAuthIntegrationTest {

    @Autowired
    protected RoleRepository roleRepository;

    @Autowired
    protected PermissionRepository permissionRepository;

    @Autowired
    protected RolePermissionRepository rolePermissionRepository;

    @Autowired
    protected UserOrganizationRoleRepository userOrganizationRoleRepository;

    @Autowired
    protected PlatformTransactionManager transactionManager;

    /** A fresh organization + an ACTIVE user (no roles yet). */
    protected RbacFixture fixture() {
        String code = "org-" + UUID.randomUUID().toString().substring(0, 8);
        String orgId = createOrganizationWithCode(code);
        User user = createUserForOrganization(orgId, uniqueEmail("rbac@"), PASSWORD);
        return new RbacFixture(UUID.fromString(orgId), code, emailOf(user), user.getId());
    }

    /** Access token of the fixture user WITHOUT any role. */
    protected String login(RbacFixture fixture) throws Exception {
        return postLoginExpectingToken(fixture.orgCode(), fixture.email(), PASSWORD);
    }

    /** Creates a role granting the given permissions. Does NOT assign it. */
    protected UUID createRole(RbacFixture fixture, String code, String name, String... permissionCodes) {
        Role role = Role.create(fixture.orgId(), code, name);
        roleRepository.saveAndFlush(role);
        grantPermissionsToRole(role.getId(), permissionCodes);
        return role.getId();
    }

    /** Creates a role granting the given permissions AND assigns it to the user. */
    protected UUID createRoleAndAssign(RbacFixture fixture, String code, String name, String... permissionCodes) {
        UUID roleId = createRole(fixture, code, name, permissionCodes);
        assignRole(fixture, roleId);
        return roleId;
    }

    protected void grantPermissionsToRole(UUID roleId, String... permissionCodes) {
        rolePermissionRepository.saveAllAndFlush(Arrays.stream(permissionCodes)
                .map(code -> RolePermission.of(roleId, code))
                .toList());
    }

    protected void replaceRolePermissions(UUID roleId, String... permissionCodes) {
        new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
            rolePermissionRepository.deleteByRoleId(roleId);
            rolePermissionRepository.saveAllAndFlush(Arrays.stream(permissionCodes)
                    .map(code -> RolePermission.of(roleId, code))
                    .toList());
        });
    }

    protected void unassignRole(RbacFixture fixture, UUID roleId) {
        new TransactionTemplate(transactionManager).executeWithoutResult(status ->
                userOrganizationRoleRepository.deleteById(new com.hiqnet.pos.rbac.domain.UserOrganizationRoleId(
                        fixture.orgId(), fixture.userId(), roleId)));
    }

    protected void assignRole(RbacFixture fixture, UUID roleId) {
        userOrganizationRoleRepository.saveAndFlush(
                UserOrganizationRole.of(fixture.orgId(), fixture.userId(), roleId));
    }

    /** Access token for a user granted EVERY current permission (fixture role). */
    protected String adminToken(RbacFixture fixture) throws Exception {
        List<String> all = permissionRepository.findAllByOrderByCodeAsc().stream()
                .map(Permission::getCode)
                .toList();
        UUID roleId = createRoleAndAssign(
                fixture, "fixture-admin", "Fixture Admin", all.toArray(String[]::new));
        return login(fixture);
    }

    protected static String bearer(String accessToken) {
        return "Bearer " + accessToken;
    }

    private static String emailOf(User user) {
        return user.getEmail();
    }

    /** Fixture record: org identity + the ACTIVE user (no roles). */
    public record RbacFixture(UUID orgId, String orgCode, String email, UUID userId) {

        public RbacFixture {
            if (orgId == null || orgCode == null || email == null || userId == null) {
                throw new IllegalArgumentException("fixture fields are required");
            }
        }
    }
}