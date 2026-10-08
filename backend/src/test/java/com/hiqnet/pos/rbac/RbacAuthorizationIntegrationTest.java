package com.hiqnet.pos.rbac;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleStatus;
import com.hiqnet.pos.users.domain.User;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.security.oauth2.jwt.JwtDecoder;

/**
 * End-to-end authorization matrix with REAL access JWTs. The same JWT must
 * reflect database changes immediately (permission removal, role assignment
 * removal, role deactivation, user/org INACTIVE) because authorization is
 * PostgreSQL-authoritative and the JWT carries identity only.
 */
class RbacAuthorizationIntegrationTest extends AbstractRbacIntegrationTest {

    @Autowired
    private JwtDecoder jwtDecoder;

    @Test
    void jwtWithoutRolesCannotReadUsers() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void userReadAllowsListButNotUpdate() throws Exception {
        RbacFixture fx = fixture();
        String token = accessTokenWith(fx, "user.read");
        UUID target = createTargetUser(fx);

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + target)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isForbidden());
    }

    @Test
    void userUpdateAllowsPatchButNotList() throws Exception {
        RbacFixture fx = fixture();
        String token = accessTokenWith(fx, "user.update");
        UUID target = createTargetUser(fx);

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + target)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void branchReadAllowsListingButNotCreation() throws Exception {
        RbacFixture fx = fixture();
        String token = accessTokenWith(fx, "branch.read");

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sucursal\",\"code\":\"suc-1\"}"))
                .andExpect(status().isForbidden());
    }

    @Test
    void organizationReadAllowsGetButNotUpdate() throws Exception {
        RbacFixture fx = fixture();
        String token = accessTokenWith(fx, "organization.read");

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId()).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isForbidden());
    }

    @Test
    void permissionRemovalIsImmediateWithTheSameJwt() throws Exception {
        RbacFixture fx = fixture();
        UUID roleId = createRole(fx, "reader", "Reader");
        grantPermissionsToRole(roleId, "user.read");
        assignRole(fx, roleId);
        String token = login(fx);

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        replaceRolePermissions(roleId); // remove user.read
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        replaceRolePermissions(roleId, "user.read"); // restore
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void roleAssignmentRemovalIsImmediateWithTheSameJwt() throws Exception {
        RbacFixture fx = fixture();
        UUID roleId = createRole(fx, "reader", "Reader", "user.read");
        assignRole(fx, roleId);
        String token = login(fx);

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        unassignRole(fx, roleId);
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void roleDeactivationIsImmediateWithTheSameJwt() throws Exception {
        RbacFixture fx = fixture();
        UUID roleId = createRole(fx, "reader", "Reader", "user.read");
        assignRole(fx, roleId);
        String token = login(fx);

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        setRoleStatus(roleId, RoleStatus.INACTIVE);
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        setRoleStatus(roleId, RoleStatus.ACTIVE);
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    @Test
    void inactiveUserIsDeniedEvenWithTheSameJwt() throws Exception {
        RbacFixture fx = fixture();
        String token = accessTokenWith(fx, "user.read");
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        User actor = userRepository.findById(fx.userId()).orElseThrow();
        actor.deactivate();
        userRepository.saveAndFlush(actor);

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void inactiveOrganizationIsDeniedEvenWithTheSameJwt() throws Exception {
        RbacFixture fx = fixture();
        String token = accessTokenWith(fx, "user.read");
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        var organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        organization.updateStatus(com.hiqnet.pos.organization.domain.OrganizationStatus.INACTIVE);
        organizationRepository.saveAndFlush(organization);

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void postOrganizationIsForbiddenEvenWithAllCurrentPermissions() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(post("/api/v1/organizations")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Other Org\",\"code\":\"other-org\","
                                + "\"timezone\":\"UTC\",\"currency\":\"MXN\",\"locale\":\"es-MX\"}"))
                .andExpect(status().isForbidden());
    }

    @Test
    void crossOrganizationScopeIsForbiddenAtTheAuthorizationBoundary() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx); // all permissions in fx.orgId

        mockMvc.perform(get(usersUrl(UUID.randomUUID())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void crossOrganizationResourceLookupIs404WhenPermissionExists() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        UUID userB = createTargetUser(fxB);

        // tokenA HAS user.read in A (authorization ok); the resource lives in B -> 404.
        mockMvc.perform(get(usersUrl(fxA.orgId()) + "/" + userB).header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void accessJwtIsIdentityOnlyWithNoRbacClaims() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        Jwt jwt = jwtDecoder.decode(token);
        assertThat(jwt.getClaimAsStringList("roles")).isNull();
        assertThat(jwt.getClaimAsStringList("permissions")).isNull();
        assertThat(jwt.getClaimAsString("token_type")).isEqualTo("access");
        assertThat(jwt.getSubject()).isEqualTo(fx.userId().toString());
        assertThat(jwt.getClaimAsString("org_id")).isEqualTo(fx.orgId().toString());
    }

    private String accessTokenWith(RbacFixture fx, String... permissionCodes) throws Exception {
        createRoleAndAssign(fx, "actor-" + UUID.randomUUID().toString().substring(0, 8), "Actor", permissionCodes);
        return login(fx);
    }

    private UUID createTargetUser(RbacFixture fx) {
        User user = User.create(fx.orgId(), uniqueEmail("target@"), passwordEncoder.encode(PASSWORD));
        userRepository.saveAndFlush(user);
        return user.getId();
    }

    private void setRoleStatus(UUID roleId, RoleStatus status) {
        Role role = roleRepository.findById(roleId).orElseThrow();
        role.updateStatus(status);
        roleRepository.saveAndFlush(role);
    }

    private static String usersUrl(UUID organizationId) {
        return "/api/v1/organizations/" + organizationId + "/users";
    }

    @Test
    void inactiveUserCanStillAuthenticateButCannotAuthorize() throws Exception {
        RbacFixture fx = fixture();
        String token = accessTokenWith(fx, "user.read");
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        User actor = userRepository.findById(fx.userId()).orElseThrow();
        actor.deactivate();
        userRepository.saveAndFlush(actor);

        // /me (identity resolution) refuses the inactive identity; /users (authz) denies.
        mockMvc.perform(get("/api/v1/auth/me").header("Authorization", bearer(token)))
                .andExpect(status().isUnauthorized());
        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }
}