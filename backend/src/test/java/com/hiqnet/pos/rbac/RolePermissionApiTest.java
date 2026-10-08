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
 * PUT /roles/{roleId}/permissions replaces the FULL set atomically: unknown
 * permissions leave the previous set untouched (404), duplicates are rejected
 * (400). The role is resolved scoped to the organization first.
 */
class RolePermissionApiTest extends AbstractRbacIntegrationTest {

    private static final String URL_TEMPLATE =
            "/api/v1/organizations/%s/roles/%s/permissions";

    @Test
    void getAndReplaceRolePermissions() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID roleId = createRole(fx, "manager", "Manager");

        // empty by default
        mockMvc.perform(get(url(fx, roleId)).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(0));

        putAndExpect(fx, roleId, token, "[\"user.read\",\"branch.read\"]", 2, "branch.read");
        putAndExpect(fx, roleId, token, "[\"branch.read\"]", 1, "branch.read");
    }

    @Test
    void putRolePermissionsRejectsUnknownPermissionLeavingSetUntouched() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID roleId = createRole(fx, "manager", "Manager", "user.read");

        mockMvc.perform(put(url(fx, roleId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"permissions\":[\"user.read\",\"not-a-permission\"]}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("PERMISSION_NOT_FOUND"));

        // atomicity: the previous set is untouched
        mockMvc.perform(get(url(fx, roleId)).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(1))
                .andExpect(jsonPath("$[0].code").value("user.read"));
    }

    @Test
    void putRolePermissionsRejectsDuplicates() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID roleId = createRole(fx, "manager", "Manager");

        mockMvc.perform(put(url(fx, roleId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"permissions\":[\"user.read\",\"user.read\"]}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void rolePermissionsOfAnotherOrganizationAreNotFound() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        UUID roleB = createRole(fxB, "manager-b", "Manager B", "user.read");

        mockMvc.perform(put(url(fxA, roleB))
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"permissions\":[\"user.read\"]}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ROLE_NOT_FOUND"));
    }

    @Test
    void putRolePermissionsWithoutManagePermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        // actor has role.read only (can read, cannot manage)
        UUID roleId = createRoleAndAssign(fx, "reader", "Reader", "role.read");
        String token = login(fx);

        mockMvc.perform(put(url(fx, roleId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"permissions\":[\"user.read\"]}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    private void putAndExpect(
            RbacFixture fx, UUID roleId, String token, String body, int expectedSize, String firstCode)
            throws Exception {
        mockMvc.perform(put(url(fx, roleId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"permissions\":" + body + "}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(expectedSize))
                .andExpect(jsonPath("$[0].code").value(firstCode));
    }

    private static String url(RbacFixture fx, UUID roleId) {
        return URL_TEMPLATE.formatted(fx.orgId(), roleId);
    }
}