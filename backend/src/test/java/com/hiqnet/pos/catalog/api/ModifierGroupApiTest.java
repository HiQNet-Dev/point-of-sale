package com.hiqnet.pos.catalog.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Modifier group administration under the REAL security chain + RBAC
 * (modifier.group.create / read / update): code is an immutable lowercase slug
 * unique per organization, selection bounds are validated, and PATCH is partial.
 */
class ModifierGroupApiTest extends AbstractRbacIntegrationTest {

    private String createGroup(String organizationId, String token, String code, String name) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(new CreateModifierGroupRequest(code, name, null, null))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createModifierGroupReturns201AndDerivedFlags() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Milk\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.code").value("milk"))
                .andExpect(jsonPath("$.name").value("Milk"))
                .andExpect(jsonPath("$.minSelections").value(0))
                .andExpect(jsonPath("$.maxSelections").value(1))
                .andExpect(jsonPath("$.required").value(false))
                .andExpect(jsonPath("$.singleChoice").value(true))
                .andExpect(jsonPath("$.multipleChoice").value(false))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void createModifierGroupWithMultipleSelectionsDerivesFlags() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"extras\",\"name\":\"Extras\",\"minSelections\":1,\"maxSelections\":3}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.minSelections").value(1))
                .andExpect(jsonPath("$.maxSelections").value(3))
                .andExpect(jsonPath("$.required").value(true))
                .andExpect(jsonPath("$.singleChoice").value(false))
                .andExpect(jsonPath("$.multipleChoice").value(true));
    }

    @Test
    void createModifierGroupWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Milk\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createModifierGroupRejectsDuplicateCodeWithinSameOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createGroup(fx.orgId().toString(), token, "milk", "Milk");
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Other\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("MODIFIER_GROUP_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createModifierGroupAllowsSameCodeInAnotherOrganization() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        createGroup(fxA.orgId().toString(), tokenA, "milk", "Milk");
        mockMvc.perform(post("/api/v1/organizations/" + fxB.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Milk\"}"))
                .andExpect(status().isCreated());
    }

    @Test
    void createModifierGroupRejectsNonSlugCode() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"Milk Shake\",\"name\":\"Milk\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void createModifierGroupRejectsMinGreaterThanMax() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"bad\",\"name\":\"Bad\",\"minSelections\":2,\"maxSelections\":1}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void getModifierGroupOfAnotherOrganizationIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String groupId = createGroup(fxA.orgId().toString(), tokenA, "milk", "Milk");
        mockMvc.perform(get("/api/v1/organizations/" + fxB.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(tokenB)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("MODIFIER_GROUP_NOT_FOUND"));
    }

    @Test
    void listModifierGroupsFiltersStatusAndQuery() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createGroup(fx.orgId().toString(), token, "milk", "Milk Type");
        createGroup(fx.orgId().toString(), token, "size", "Size");

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .param("q", "milk"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].code").value("milk"));

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .param("status", "INACTIVE"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(0));
    }

    @Test
    void patchModifierGroupUpdatesFieldsAndBumpsVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk", "Milk");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Milk Type\",\"minSelections\":1,"
                                + "\"maxSelections\":2,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Milk Type"))
                .andExpect(jsonPath("$.minSelections").value(1))
                .andExpect(jsonPath("$.maxSelections").value(2))
                .andExpect(jsonPath("$.required").value(true))
                .andExpect(jsonPath("$.multipleChoice").value(true))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.code").value("milk"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchModifierGroupIgnoresCodeInBody() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk", "Milk");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"code\":\"hacked\",\"name\":\"Milk 2\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("milk"))
                .andExpect(jsonPath("$.name").value("Milk 2"));
    }

    @Test
    void patchModifierGroupStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk", "Milk");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Milk 2\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }
}
