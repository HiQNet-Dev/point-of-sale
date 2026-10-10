package com.hiqnet.pos.catalog.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Modifier option administration nested under a modifier group: code is a
 * lowercase slug unique within the group, {@code priceDelta} is a signed
 * decimal (NUMERIC(19,4)) and options are ordered by displayOrder.
 */
class ModifierOptionApiTest extends AbstractRbacIntegrationTest {

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private ModifierGroupRepository modifierGroupRepository;

    private ModifierGroup modifierGroupEntity(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return modifierGroupRepository.saveAndFlush(ModifierGroup.create(organization, code, "Group " + code, 0, 1));
    }

    private String createGroup(String organizationId, String token, String code) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"Group\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createOption(
            String organizationId, String groupId, String token, String code, String priceDelta, int order)
            throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/modifier-groups/"
                                + groupId + "/options")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"Option\",\"priceDelta\":" + priceDelta
                                + ",\"displayOrder\":" + order + "}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createModifierOptionReturns201AndAcceptsNegativeDelta() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId + "/options")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat\",\"priceDelta\":-1.5000,\"displayOrder\":0}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.modifierGroupId").value(groupId))
                .andExpect(jsonPath("$.code").value("oat"))
                .andExpect(jsonPath("$.priceDelta").value(-1.5))
                .andExpect(jsonPath("$.displayOrder").value(0))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void createModifierOptionWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String groupId = modifierGroupEntity(fx, "milk").getId().toString();
        String readToken = login(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId + "/options")
                        .header("Authorization", bearer(readToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createModifierOptionRejectsDuplicateCodeWithinGroup() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        createOption(fx.orgId().toString(), groupId, token, "oat", "0", 0);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId + "/options")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat 2\",\"priceDelta\":0,\"displayOrder\":1}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("MODIFIER_OPTION_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createModifierOptionAllowsSameCodeInAnotherGroup() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        String otherGroupId = createGroup(fx.orgId().toString(), token, "size");
        createOption(fx.orgId().toString(), groupId, token, "large", "0", 0);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + otherGroupId + "/options")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"large\",\"name\":\"Large\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isCreated());
    }

    @Test
    void createModifierOptionUnderUnknownGroupIs404() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/"
                                + java.util.UUID.randomUUID() + "/options")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("MODIFIER_GROUP_NOT_FOUND"));
    }

    @Test
    void createModifierOptionUnderAnotherOrganizationGroupIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String groupId = createGroup(fxA.orgId().toString(), tokenA, "milk");
        mockMvc.perform(post("/api/v1/organizations/" + fxB.orgId() + "/modifier-groups/" + groupId + "/options")
                        .header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void listModifierOptionsOrdersByDisplayOrder() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        createOption(fx.orgId().toString(), groupId, token, "second", "1", 2);
        createOption(fx.orgId().toString(), groupId, token, "first", "0", 1);
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId + "/options")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(2))
                .andExpect(jsonPath("$.items[0].code").value("first"))
                .andExpect(jsonPath("$.items[1].code").value("second"));
    }

    @Test
    void patchModifierOptionUpdatesFieldsAndBumpsVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        String optionId = createOption(fx.orgId().toString(), groupId, token, "oat", "0", 0);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId
                                + "/options/" + optionId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Oat milk\",\"priceDelta\":2.2500,"
                                + "\"displayOrder\":5,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Oat milk"))
                .andExpect(jsonPath("$.priceDelta").value(2.25))
                .andExpect(jsonPath("$.displayOrder").value(5))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.code").value("oat"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchModifierOptionStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        String optionId = createOption(fx.orgId().toString(), groupId, token, "oat", "0", 0);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId
                                + "/options/" + optionId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Oat 2\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId
                                + "/options/" + optionId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void getModifierOptionOfAnotherGroupIs404() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        String otherGroupId = createGroup(fx.orgId().toString(), token, "size");
        String optionId = createOption(fx.orgId().toString(), groupId, token, "oat", "0", 0);
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + otherGroupId
                                + "/options/" + optionId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("MODIFIER_OPTION_NOT_FOUND"));
    }
}
