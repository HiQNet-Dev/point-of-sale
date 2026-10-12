package com.hiqnet.pos.inventory.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

class InventoryItemApiTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired
    private InventoryItemService inventoryItemService;

    private String createItem(UUID organizationId, String token, String code, String name, String unit)
            throws Exception {
        MvcResult result = mockMvc.perform(post(BASE + organizationId + "/inventory-items")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + name
                                + "\",\"baseUnit\":\"" + unit + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        return objectMapper.readTree(result.getResponse().getContentAsString()).get("id").asText();
    }

    @Test
    void inventoryRoutesRequireAuthentication() throws Exception {
        RbacFixture fx = fixture();
        mockMvc.perform(get(BASE + fx.orgId() + "/inventory-items"))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void createReturnsOrganizationOwnedActiveItemAndRecordsAudit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(BASE + fx.orgId() + "/inventory-items")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"whole-milk\",\"name\":\"  Whole Milk  \",\"description\":\"  Beverage ingredient  \",\"baseUnit\":\"MILLILITER\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.code").value("whole-milk"))
                .andExpect(jsonPath("$.name").value("Whole Milk"))
                .andExpect(jsonPath("$.description").value("Beverage ingredient"))
                .andExpect(jsonPath("$.baseUnit").value("MILLILITER"))
                .andExpect(jsonPath("$.fractionalUnitAllowed").value(false))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty())
                .andExpect(jsonPath("$.updatedAt").isNotEmpty());

        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("inventory.item.created");
        assertThat(latestAudit(fx.orgId()).entityType()).isEqualTo("inventory_item");
        assertThat(metadataOf(latestAudit(fx.orgId()))).containsEntry("code", "whole-milk");
    }

    @Test
    void createRejectsInvalidCodeAndBlankName() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String url = BASE + fx.orgId() + "/inventory-items";
        mockMvc.perform(post(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"Whole_Milk\",\"name\":\"Milk\",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
        mockMvc.perform(post(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"   \",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void duplicateCodesAreOrganizationScopedAndCrossOrganizationReuseIsAllowed() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        createItem(fxA.orgId(), tokenA, "milk", "Milk A", "GRAM");
        mockMvc.perform(post(BASE + fxA.orgId() + "/inventory-items")
                        .header("Authorization", bearer(tokenA)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Duplicate\",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INVENTORY_ITEM_CODE_ALREADY_EXISTS"));
        mockMvc.perform(post(BASE + fxB.orgId() + "/inventory-items")
                        .header("Authorization", bearer(tokenB)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Milk B\",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isCreated());
    }

    @Test
    void listFiltersByQueryStatusAndBaseUnitWithStablePagination() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String milkId = createItem(fx.orgId(), token, "whole-milk", "Whole Milk", "MILLILITER");
        String beansId = createItem(fx.orgId(), token, "coffee-beans", "Coffee Beans", "GRAM");
        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + milkId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(get(BASE + fx.orgId() + "/inventory-items")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].name").value("Coffee Beans"))
                .andExpect(jsonPath("$.items[1].name").value("Whole Milk"));
        mockMvc.perform(get(BASE + fx.orgId() + "/inventory-items")
                        .header("Authorization", bearer(token)).param("q", "COFFEE")
                        .param("status", "ACTIVE").param("baseUnit", "GRAM").param("page", "0").param("size", "1"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].id").value(beansId))
                .andExpect(jsonPath("$.page").value(0))
                .andExpect(jsonPath("$.size").value(1))
                .andExpect(jsonPath("$.totalElements").value(1));
        mockMvc.perform(get(BASE + fx.orgId() + "/inventory-items")
                        .header("Authorization", bearer(token)).param("status", "INACTIVE"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].id").value(milkId));
    }

    @Test
    void getAndUpdateAreOrganizationScoped() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String itemB = createItem(fxB.orgId(), tokenB, "milk", "Milk", "GRAM");

        mockMvc.perform(get(BASE + fxA.orgId() + "/inventory-items/" + itemB)
                        .header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("INVENTORY_ITEM_NOT_FOUND"));
        mockMvc.perform(patch(BASE + fxA.orgId() + "/inventory-items/" + itemB)
                        .header("Authorization", bearer(tokenA)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Tampered\"}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void updateAllowsMetadataCorrectionAndStatusTransitionWithAudit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String itemId = createItem(fx.orgId(), token, "milk", "Milk", "GRAM");
        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"  Whole Milk  \",\"description\":\"  \",\"baseUnit\":\"MILLILITER\",\"fractionalUnitAllowed\":true,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("milk"))
                .andExpect(jsonPath("$.name").value("Whole Milk"))
                .andExpect(jsonPath("$.description").value(org.hamcrest.Matchers.nullValue()))
                .andExpect(jsonPath("$.baseUnit").value("MILLILITER"))
                .andExpect(jsonPath("$.fractionalUnitAllowed").value(true))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(1));
        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("inventory.item.updated");
        assertThat(metadataOf(latestAudit(fx.orgId())).get("changedFields").toString())
                .contains("name", "description", "baseUnit", "fractionalUnitAllowed", "status");
    }

    @Test
    void noOpUpdateDoesNotIncrementVersionOrWriteAuditAndStaleUpdateConflicts() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String itemId = createItem(fx.orgId(), token, "milk", "Milk", "GRAM");
        long auditBefore = auditCount(fx.orgId());
        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(0));
        assertThat(auditCount(fx.orgId())).isEqualTo(auditBefore);

        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Milk 2\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void readAndWritePermissionsAreIndependentAndOrganizationScoped() throws Exception {
        RbacFixture reader = fixture();
        String itemId = inventoryItemService.create(
                reader.orgId(), "milk", "Milk", null, InventoryBaseUnit.GRAM, false).getId().toString();
        createRoleAndAssign(reader, "inventory-reader", "Inventory Reader", "inventory.item.read");
        String readerToken = login(reader);
        String listUrl = BASE + reader.orgId() + "/inventory-items";
        mockMvc.perform(get(listUrl).header("Authorization", bearer(readerToken)))
                .andExpect(status().isOk());
        mockMvc.perform(get(listUrl + "/" + itemId).header("Authorization", bearer(readerToken)))
                .andExpect(status().isOk());
        mockMvc.perform(post(listUrl).header("Authorization", bearer(readerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"salt\",\"name\":\"Salt\",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isForbidden());
        mockMvc.perform(patch(listUrl + "/" + itemId).header("Authorization", bearer(readerToken))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"name\":\"No\"}"))
                .andExpect(status().isForbidden());

        RbacFixture writer = fixture();
        createRoleAndAssign(writer, "inventory-writer", "Inventory Writer", "inventory.item.write");
        String writerToken = login(writer);
        MvcResult writerCreated = mockMvc.perform(post(BASE + writer.orgId() + "/inventory-items")
                        .header("Authorization", bearer(writerToken)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"salt\",\"name\":\"Salt\",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        String writerItemId = objectMapper.readTree(writerCreated.getResponse().getContentAsString())
                .get("id").asText();
        mockMvc.perform(patch(BASE + writer.orgId() + "/inventory-items/" + writerItemId)
                        .header("Authorization", bearer(writerToken)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Salt Fine\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(get(BASE + writer.orgId() + "/inventory-items")
                        .header("Authorization", bearer(writerToken)))
                .andExpect(status().isForbidden());
    }

    @Test
    void readsDoNotWriteAuditEvents() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String itemId = createItem(fx.orgId(), token, "milk", "Milk", "GRAM");
        long before = auditCount(fx.orgId());
        mockMvc.perform(get(BASE + fx.orgId() + "/inventory-items").header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        mockMvc.perform(get(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }
}
