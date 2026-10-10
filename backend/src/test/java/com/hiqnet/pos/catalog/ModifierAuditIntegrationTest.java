package com.hiqnet.pos.catalog;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Every modifier mutation writes EXACTLY ONE append-only audit event inside the
 * business transaction; a no-op replacement writes none.
 */
class ModifierAuditIntegrationTest extends AbstractAuditIntegrationTest {

    private String createGroup(String organizationId, String token, String code) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"Group " + code + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createProduct(String organizationId, String token, String sku) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"" + sku + "\",\"name\":\"" + sku + "\",\"unitPrice\":10.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createGroupRecordsAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("modifier.group.created");
        assertThat(row.entityType()).isEqualTo("modifier_group");
        assertThat(row.entityId()).hasToString(groupId);
        assertThat(metadataOf(row)).containsEntry("code", "milk");
    }

    @Test
    void updateGroupRecordsChangedFields() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Milk Type\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("modifier.group.updated");
        assertThat(row.entityType()).isEqualTo("modifier_group");
        @SuppressWarnings("unchecked")
        List<String> changedFields = (List<String>) metadataOf(row).get("changedFields");
        assertThat(changedFields).containsExactlyInAnyOrder("name", "status");
    }

    @Test
    void createOptionRecordsAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String groupId = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + groupId + "/options")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isCreated());

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("modifier.option.created");
        assertThat(row.entityType()).isEqualTo("modifier_option");
        assertThat(metadataOf(row)).containsEntry("code", "oat");
    }

    @Test
    void replaceRecordsModifierGroupCount() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        String size = createGroup(fx.orgId().toString(), token, "size");
        mockMvc.perform(put("/api/v1/organizations/" + fx.orgId() + "/products/" + productId + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\",\"" + size + "\"]}"))
                .andExpect(status().isOk());

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("product.modifier-groups.replaced");
        assertThat(row.entityType()).isEqualTo("product");
        assertThat(row.entityId()).hasToString(productId);
        Map<String, Object> metadata = metadataOf(row);
        assertThat(metadata).containsEntry("modifierGroupCount", 2);
    }

    @Test
    void noOpReplacementWritesNoAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        String url = "/api/v1/organizations/" + fx.orgId() + "/products/" + productId + "/modifier-groups";
        String body = "{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}";
        mockMvc.perform(put(url)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk());
        long before = auditCount(fx.orgId());
        mockMvc.perform(put(url)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isOk());
        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }
}
