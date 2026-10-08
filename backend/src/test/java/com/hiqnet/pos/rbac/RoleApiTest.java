package com.hiqnet.pos.rbac;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Role administration under the REAL security chain + RBAC (role.create /
 * role.read / role.update). Role code is a normalized immutable slug; no
 * DELETE (deactivation is status=INACTIVE); PATCH requires expectedVersion.
 */
class RoleApiTest extends AbstractRbacIntegrationTest {

    @Test
    void createRoleReturns201AndDto() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(rolesUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"store-manager\",\"name\":\"Store Manager\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.code").value("store-manager"))
                .andExpect(jsonPath("$.name").value("Store Manager"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty());
    }

    @Test
    void createRoleWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        mockMvc.perform(post(rolesUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"store-manager\",\"name\":\"Store Manager\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createRoleRejectsNonSlugCode() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(rolesUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"Store Manager!\",\"name\":\"Store Manager\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void createRoleRejectsDuplicateCodeWithinOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(rolesUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"store-manager\",\"name\":\"One\"}"))
                .andExpect(status().isCreated());
        mockMvc.perform(post(rolesUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"store-manager\",\"name\":\"Two\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("ROLE_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createRoleAllowsSameCodeInDifferentOrganizations() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        mockMvc.perform(post(rolesUrl(fxA.orgId())).header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"cashier\",\"name\":\"Cashier\"}"))
                .andExpect(status().isCreated());
        mockMvc.perform(post(rolesUrl(fxB.orgId())).header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"cashier\",\"name\":\"Cashier\"}"))
                .andExpect(status().isCreated());
    }

    @Test
    void getRoleWithinOrganizationReturns200() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID roleId = createRole(fx, "store-manager", "Store Manager", "user.read");
        mockMvc.perform(get(rolesUrl(fx.orgId()) + "/" + roleId).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("store-manager"));
    }

    @Test
    void getRoleOfAnotherOrganizationReturns404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        UUID roleB = createRole(fxB, "manager-b", "Manager B");

        mockMvc.perform(get(rolesUrl(fxA.orgId()) + "/" + roleB).header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ROLE_NOT_FOUND"));
    }

    @Test
    void listRolesIsPaginatedAndOrderedByCode() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createRole(fx, "zeta-role", "Zeta", "user.read");
        createRole(fx, "alpha-role", "Alpha", "user.read");
        createRole(fx, "mid-role", "Mid", "user.read");

        mockMvc.perform(get(rolesUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(4)) // includes fixture-admin
                .andExpect(jsonPath("$.items[0].code").value("alpha-role"))
                .andExpect(jsonPath("$.items[1].code").value("fixture-admin"))
                .andExpect(jsonPath("$.items[2].code").value("mid-role"))
                .andExpect(jsonPath("$.items[3].code").value("zeta-role"));
    }

    @Test
    void listRolesRejectsInvalidPaging() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(get(rolesUrl(fx.orgId())).param("size", "101").header("Authorization", bearer(token)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listRolesFiltersByStatus() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID roleId = createRole(fx, "off-role", "Off", "user.read");
        patchRole(fx, token, roleId, "{\"expectedVersion\":0,\"status\":\"INACTIVE\"}");

        mockMvc.perform(get(rolesUrl(fx.orgId())).param("status", "INACTIVE").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].code").value("off-role"));
    }

    @Test
    void patchRoleUpdatesNameAndStatusAndKeepsCodeImmutable() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID roleId = createRole(fx, "store-manager", "Store Manager", "user.read");

        mockMvc.perform(patch(rolesUrl(fx.orgId()) + "/" + roleId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Store Manager North\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Store Manager North"))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.code").value("store-manager"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchRoleStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID roleId = createRole(fx, "store-manager", "Store Manager", "user.read");

        mockMvc.perform(patch(rolesUrl(fx.orgId()) + "/" + roleId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Renamed\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(patch(rolesUrl(fx.orgId()) + "/" + roleId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    private void patchRole(RbacFixture fx, String token, UUID roleId, String body) throws Exception {
        mockMvc.perform(patch(rolesUrl(fx.orgId()) + "/" + roleId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk());
    }

    private static String rolesUrl(UUID organizationId) {
        return "/api/v1/organizations/" + organizationId + "/roles";
    }
}