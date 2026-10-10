package com.hiqnet.pos.rbac;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * The system permission catalog is seeded by V5 and is NOT a client CRUD.
 * Integrity: exactly the documented codes, no duplicates, ordered listing,
 * no mutation endpoint.
 */
class PermissionCatalogTest extends AbstractRbacIntegrationTest {

    private static final List<String> EXPECTED_CODES = List.of(
            "audit.read",
            "branch.create",
            "branch.membership.create",
            "branch.membership.read",
            "branch.membership.roles.manage",
            "branch.membership.update",
            "branch.read",
            "branch.update",
            "category.create",
            "category.read",
            "category.update",
            "organization.read",
            "organization.update",
            "permission.read",
            "product.create",
            "product.read",
            "product.update",
            "role.assign",
            "role.create",
            "role.permissions.manage",
            "role.read",
            "role.update",
            "user.create",
            "user.read",
            "user.update");

    @Test
    void catalogContainsExactlyTheSeededPermissions() {
        List<String> codes = permissionRepository.findAllByOrderByCodeAsc().stream()
                .map(p -> p.getCode())
                .toList();
        assertThat(codes).containsExactlyElementsOf(EXPECTED_CODES);
    }

    @Test
    void catalogIsListedOrderedWithPermissionRead() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/permissions")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(EXPECTED_CODES.size()))
                .andExpect(jsonPath("$[0].code").value("audit.read"))
                .andExpect(jsonPath("$[1].code").value("branch.create"))
                .andExpect(jsonPath("$[3].code").value("branch.membership.read"))
                .andExpect(jsonPath("$[13].code").value("permission.read"))
                .andExpect(jsonPath("$[24].code").value("user.update"));
    }

    @Test
    void catalogIsForbiddenWithoutPermissionRead() throws Exception {
        RbacFixture fx = fixture();
        UUID roleId = createRoleAndAssign(fx, "reader", "Reader", "user.read");
        String token = login(fx);

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/permissions")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void catalogHasNoClientMutationEndpoint() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/permissions")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"super.new\",\"description\":\"x\"}"))
                .andExpect(status().isForbidden());
    }
}