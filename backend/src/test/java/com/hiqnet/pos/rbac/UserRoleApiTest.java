package com.hiqnet.pos.rbac;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * PUT /users/{userId}/roles replaces the user's organization role set
 * atomically; every role must belong to the same organization (404 before any
 * mutation). Assigning INACTIVE roles is allowed (they grant nothing while
 * INACTIVE; assignments survive).
 */
class UserRoleApiTest extends AbstractRbacIntegrationTest {

    private static final String URL_TEMPLATE =
            "/api/v1/organizations/%s/users/%s/roles";

    @Test
    void getAndReplaceUserRoles() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID target = createTargetUser(fx);
        UUID roleA = createRole(fx, "role-a", "Role A", "user.read");
        UUID roleB = createRole(fx, "role-b", "Role B", "user.read");
        UUID roleC = createRole(fx, "role-c", "Role C", "user.read");

        putRolesAndExpect(fx, target, token, new UUID[] {roleA, roleB});
        putRolesAndExpect(fx, target, token, new UUID[] {roleB, roleC});

        mockMvc.perform(get(userRolesUrlFor(fx.orgId(), target)).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(2))
                .andExpect(jsonPath("$[0].code").value("role-b"))
                .andExpect(jsonPath("$[1].code").value("role-c"));
    }

    @Test
    void putUserRolesRejectsDuplicates() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID target = createTargetUser(fx);
        UUID roleA = createRole(fx, "role-a", "Role A", "user.read");

        mockMvc.perform(put(userRolesUrlFor(fx.orgId(), target))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[\"" + roleA + "\",\"" + roleA + "\"]}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void putUserRolesWithCrossOrganizationRoleRollsBack() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        UUID target = createTargetUser(fxA);
        UUID roleA = createRole(fxA, "role-a", "Role A", "user.read");
        UUID roleB = createRole(fxB, "role-b", "Role B", "user.read");

        // Prime the target with roleA (in A), then try to add B's role
        // (cross-tenant) -> 404 + rollback.
        mockMvc.perform(put(userRolesUrlFor(fxA.orgId(), target))
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[\"" + roleA + "\"]}"))
                .andExpect(status().isOk());

        mockMvc.perform(put(userRolesUrlFor(fxA.orgId(), target))
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[\"" + roleA + "\",\"" + roleB + "\"]}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ROLE_NOT_FOUND"));

        // Atomic: previous set untouched.
        mockMvc.perform(get(userRolesUrlFor(fxA.orgId(), target)).header("Authorization", bearer(tokenA)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(1))
                .andExpect(jsonPath("$[0].code").value("role-a"));
    }

    @Test
    void putUserRolesForUnknownUserIsNotFound() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(put(userRolesUrlFor(fx.orgId(), UUID.randomUUID()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[]}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void getUserRolesOfAnotherOrganizationUserIsNotFound() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);

        mockMvc.perform(get(userRolesUrlFor(fxA.orgId(), fxB.userId()))
                        .header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void putUserRolesWithoutAssignPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        UUID roleId = createRoleAndAssign(fx, "reader", "Reader", "role.read");
        String token = login(fx);

        mockMvc.perform(put(userRolesUrlFor(fx.orgId(), fx.userId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[\"" + roleId + "\"]}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    private UUID createTargetUser(RbacFixture fx) {
        com.hiqnet.pos.users.domain.User user = com.hiqnet.pos.users.domain.User.create(
                fx.orgId(), uniqueEmail("roletarget@"), passwordEncoder.encode(PASSWORD));
        userRepository.saveAndFlush(user);
        return user.getId();
    }

    private void putRolesAndExpect(RbacFixture fx, UUID target, String token, UUID[] roleIds) throws Exception {
        StringBuilder body = new StringBuilder("{\"roleIds\":[");
        for (int i = 0; i < roleIds.length; i++) {
            if (i > 0) {
                body.append(',');
            }
            body.append('"').append(roleIds[i]).append('"');
        }
        body.append("]}");
        mockMvc.perform(put(userRolesUrlFor(fx.orgId(), target))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body.toString()))
                .andExpect(status().isOk());
    }

    private static String userRolesUrlFor(UUID orgId, UUID userId) {
        return URL_TEMPLATE.formatted(orgId, userId);
    }
}