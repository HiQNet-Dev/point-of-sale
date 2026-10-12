package com.hiqnet.pos.inventory.modifier;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

class ModifierInventoryEffectsApiTest extends AbstractAuditIntegrationTest {
    private static final String BASE = "/api/v1/organizations/";
    @Autowired private InventoryItemService itemService;
    @Autowired private JdbcTemplate jdbc;

    private String option(RbacFixture fx, String token) throws Exception {
        String group = objectMapper.readTree(mockMvc.perform(post(BASE + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk-" + UUID.randomUUID().toString().substring(0, 6)
                                + "\",\"name\":\"Milk\"}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString()).get("id").asText();
        return objectMapper.readTree(mockMvc.perform(post(BASE + fx.orgId() + "/modifier-groups/" + group + "/options")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString()).get("id").asText();
    }

    private String item(RbacFixture fx, String unit, boolean fractional) {
        return itemService.create(fx.orgId(), "item-" + UUID.randomUUID().toString().substring(0, 8),
                "Item", null, InventoryBaseUnit.valueOf(unit), fractional).getId().toString();
    }

    private String url(RbacFixture fx, String optionId) {
        return BASE + fx.orgId() + "/modifier-options/" + optionId + "/inventory-effects";
    }

    @Test
    void replacesSignedEffectsReturnsExactStringsAndTreatsEmptyAsValid() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String optionId = option(fx, token);
        String milkId = item(fx, "MILLILITER", false);
        String oatId = item(fx, "MILLILITER", false);
        String url = url(fx, optionId);
        mockMvc.perform(get(url).header("Authorization", bearer(token)))
                .andExpect(status().isNotFound()).andExpect(jsonPath("$.code").value("INVENTORY_MODIFIER_CONFIG_NOT_FOUND"));
        long before = auditCount(fx.orgId());
        String body = "{\"expectedVersion\":0,\"effects\":["
                + "{\"inventoryItemId\":\"" + milkId + "\",\"quantityDelta\":\"-250\"},"
                + "{\"inventoryItemId\":\"" + oatId + "\",\"quantityDelta\":\"250.5\"}]}";
        String createdResponse = mockMvc.perform(put(url).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isOk()).andExpect(jsonPath("$.version").value(1))
                .andReturn().getResponse().getContentAsString();
        var createdEffects = objectMapper.readTree(createdResponse).get("effects");
        assertThat(java.util.List.of(
                        createdEffects.get(0).get("inventoryItemId").asText() + "="
                                + createdEffects.get(0).get("quantityDelta").asText(),
                        createdEffects.get(1).get("inventoryItemId").asText() + "="
                                + createdEffects.get(1).get("quantityDelta").asText()))
                .containsExactlyInAnyOrder(milkId + "=-250.000000", oatId + "=250.500000");
        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("inventory.modifier_effects.updated");
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
        long afterCreate = auditCount(fx.orgId());
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"effects\":["
                                + "{\"inventoryItemId\":\"" + oatId + "\",\"quantityDelta\":\"250.500000\"},"
                                + "{\"inventoryItemId\":\"" + milkId + "\",\"quantityDelta\":\"-250.000000\"}]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.version").value(1));
        assertThat(auditCount(fx.orgId())).isEqualTo(afterCreate);

        String emptyOptionId = option(fx, token);
        String emptyUrl = url(fx, emptyOptionId);
        mockMvc.perform(put(emptyUrl).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"effects\":[]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.version").value(1))
                .andExpect(jsonPath("$.effects.length()").value(0));
        long beforeNoOp = auditCount(fx.orgId());
        mockMvc.perform(put(emptyUrl).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"effects\":[]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.version").value(1));
        assertThat(auditCount(fx.orgId())).isEqualTo(beforeNoOp);
        assertThat(auditCount(fx.orgId())).isGreaterThan(before);
    }

    @Test
    void inactiveInventoryReferencesRemainReadableButCannotBeChangedOrAdded() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String optionId = option(fx, token);
        String itemId = item(fx, "GRAM", false);
        String url = url(fx, optionId);
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"effects\":[{\"inventoryItemId\":\""
                                + itemId + "\",\"quantityDelta\":\"1\"}]}"))
                .andExpect(status().isOk());
        jdbc.update("update inventory_items set status = 'INACTIVE' where id = ?", UUID.fromString(itemId));
        mockMvc.perform(get(url).header("Authorization", bearer(token))).andExpect(status().isOk())
                .andExpect(jsonPath("$.effects[0].status").value("INACTIVE"));
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"effects\":[{\"inventoryItemId\":\""
                                + itemId + "\",\"quantityDelta\":\"2\"}]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("INVENTORY_ITEM_INACTIVE"));
    }

    @Test
    void postgresEnforcesUniqueNonzeroAndSameOrganizationEffects() throws Exception {
        RbacFixture fx = fixture();
        RbacFixture other = fixture();
        String token = adminToken(fx);
        String optionId = option(fx, token);
        String itemId = item(fx, "GRAM", false);
        String otherItemId = item(other, "GRAM", false);
        String url = url(fx, optionId);
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"effects\":[]}"))
                .andExpect(status().isOk());
        UUID configId = jdbc.queryForObject("select id from modifier_option_inventory_configs where modifier_option_id = ?",
                UUID.class, UUID.fromString(optionId));
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"effects\":[{\"inventoryItemId\":\""
                                + otherItemId + "\",\"quantityDelta\":\"1\"}]}"))
                .andExpect(status().isNotFound()).andExpect(jsonPath("$.code").value("INVENTORY_ITEM_NOT_FOUND"));
        assertThatThrownBy(() -> jdbc.update("insert into modifier_option_inventory_effects "
                        + "(id,organization_id,config_id,inventory_item_id,quantity_delta) values (?,?,?,?,?)",
                        UUID.randomUUID(), fx.orgId(), configId, UUID.fromString(otherItemId), new java.math.BigDecimal("1")))
                .isInstanceOf(org.springframework.dao.DataIntegrityViolationException.class);
        jdbc.update("insert into modifier_option_inventory_effects "
                        + "(id,organization_id,config_id,inventory_item_id,quantity_delta) values (?,?,?,?,?)",
                UUID.randomUUID(), fx.orgId(), configId, UUID.fromString(itemId), new java.math.BigDecimal("1"));
        assertThatThrownBy(() -> jdbc.update("insert into modifier_option_inventory_effects "
                        + "(id,organization_id,config_id,inventory_item_id,quantity_delta) values (?,?,?,?,?)",
                        UUID.randomUUID(), fx.orgId(), configId, UUID.fromString(itemId), new java.math.BigDecimal("2")))
                .isInstanceOf(org.springframework.dao.DataIntegrityViolationException.class);
        assertThatThrownBy(() -> jdbc.update("insert into modifier_option_inventory_effects "
                        + "(id,organization_id,config_id,inventory_item_id,quantity_delta) values (?,?,?,?,?)",
                        UUID.randomUUID(), fx.orgId(), configId, UUID.randomUUID(), new java.math.BigDecimal("0")))
                .isInstanceOf(org.springframework.dao.DataIntegrityViolationException.class);
    }

    @Test
    void rejectsZeroNumericDuplicateAndFractionalDiscreteEffects() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String optionId = option(fx, token);
        String discreteId = item(fx, "UNIT", false);
        String url = url(fx, optionId);
        for (String invalid : new String[] {"0", "0.5"}) {
            mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                            .content("{\"expectedVersion\":0,\"effects\":[{\"inventoryItemId\":\""
                                    + discreteId + "\",\"quantityDelta\":\"" + invalid + "\"}]}"))
                    .andExpect(status().isBadRequest());
        }
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"effects\":[{\"inventoryItemId\":\""
                                + discreteId + "\",\"quantityDelta\":1}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_INVENTORY_QUANTITY"));
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"effects\":["
                                + "{\"inventoryItemId\":\"" + discreteId + "\",\"quantityDelta\":\"1\"},"
                                + "{\"inventoryItemId\":\"" + discreteId + "\",\"quantityDelta\":\"-1\"}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }
}
