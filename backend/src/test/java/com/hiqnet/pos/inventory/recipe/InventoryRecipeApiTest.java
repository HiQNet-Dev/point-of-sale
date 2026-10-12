package com.hiqnet.pos.inventory.recipe;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;


import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.inventory.consumption.InventoryConsumptionResolver;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.consumption.ResolvedInventoryConsumption;

import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

class InventoryRecipeApiTest extends AbstractAuditIntegrationTest {
    private static final String BASE = "/api/v1/organizations/";

    @Autowired private InventoryItemService inventoryItemService;
    @Autowired private InventoryConsumptionResolver resolver;
    @Autowired private JdbcTemplate jdbc;

    private String product(RbacFixture fx, String token, String sku) throws Exception {
        String body = mockMvc.perform(post(BASE + fx.orgId() + "/products")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"" + sku + "\",\"name\":\"" + sku + "\",\"unitPrice\":10.0000}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString();
        return objectMapper.readTree(body).get("id").asText();
    }

    private String item(RbacFixture fx, String code, String unit, boolean fractional) {
        return inventoryItemService.create(fx.orgId(), code, code, null, InventoryBaseUnit.valueOf(unit), fractional)
                .getId().toString();
    }

    private String recipeUrl(RbacFixture fx, String productId) {
        return BASE + fx.orgId() + "/products/" + productId + "/inventory-recipe";
    }

    private String modifierOption(RbacFixture fx, String token, String suffix) throws Exception {
        String groupBody = mockMvc.perform(post(BASE + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"recipe-group-" + suffix + "\",\"name\":\"Recipe group\"}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString();
        String groupId = objectMapper.readTree(groupBody).get("id").asText();
        String optionBody = mockMvc.perform(post(BASE + fx.orgId() + "/modifier-groups/" + groupId + "/options")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"option-" + suffix + "\",\"name\":\"Option\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString();
        return objectMapper.readTree(optionBody).get("id").asText();
    }

    private void replaceEffects(RbacFixture fx, String token, String optionId, String effects) throws Exception {
        mockMvc.perform(put(BASE + fx.orgId() + "/modifier-options/" + optionId + "/inventory-effects")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"effects\":" + effects + "}"))
                .andExpect(status().isOk());
    }

    @Test
    void getMissingRecipeAndReplaceActiveRecipeWithExactStringsAndNoOpVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = product(fx, token, "LATTE");
        String coffeeId = item(fx, "coffee-" + UUID.randomUUID().toString().substring(0, 8), "GRAM", false);
        String milkId = item(fx, "milk-" + UUID.randomUUID().toString().substring(0, 8), "MILLILITER", false);
        String url = recipeUrl(fx, productId);

        mockMvc.perform(get(url).header("Authorization", bearer(token)))
                .andExpect(status().isNotFound()).andExpect(jsonPath("$.code").value("INVENTORY_RECIPE_NOT_FOUND"));
        long auditsBefore = auditCount(fx.orgId());
        String body = "{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                + "{\"inventoryItemId\":\"" + coffeeId + "\",\"quantity\":\"18.5\"},"
                + "{\"inventoryItemId\":\"" + milkId + "\",\"quantity\":\"250\"}]}";
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isOk()).andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(1)).andExpect(jsonPath("$.components.length()").value(2));
        assertThat(auditCount(fx.orgId())).isEqualTo(auditsBefore + 1);
        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("inventory.recipe.created");
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));

        String reversed = "{\"expectedVersion\":1,\"status\":\"ACTIVE\",\"components\":["
                + "{\"inventoryItemId\":\"" + milkId + "\",\"quantity\":\"250.000000\"},"
                + "{\"inventoryItemId\":\"" + coffeeId + "\",\"quantity\":\"18.500000\"}]}";
        long beforeNoOp = auditCount(fx.orgId());
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(reversed))
                .andExpect(status().isOk()).andExpect(jsonPath("$.version").value(1))
                .andExpect(jsonPath("$.components[0].quantity").value("18.500000"));
        assertThat(auditCount(fx.orgId())).isEqualTo(beforeNoOp);

        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"INACTIVE\",\"components\":[]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(2));
        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("inventory.recipe.status_changed");
        assertThat(resolver.resolve(fx.orgId(), UUID.fromString(productId), java.util.List.of()).tracked()).isFalse();
    }

    @Test
    void rejectsActiveEmptyDuplicateNumericAndDiscreteFractionalComponents() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = product(fx, token, "UNITTEST");
        String cupId = item(fx, "cup-" + UUID.randomUUID().toString().substring(0, 8), "UNIT", false);
        String url = recipeUrl(fx, productId);
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":[]}"))
                .andExpect(status().isBadRequest());
        for (String invalidQuantity : new String[] {"0", "-1"}) {
            mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                            .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                    + "{\"inventoryItemId\":\"" + cupId + "\",\"quantity\":\""
                                    + invalidQuantity + "\"}]}"))
                    .andExpect(status().isBadRequest());
        }
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + cupId + "\",\"quantity\":0.5}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_INVENTORY_QUANTITY"));
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + cupId + "\",\"quantity\":\"0.5\"}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_INVENTORY_QUANTITY"));
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + cupId + "\",\"quantity\":\"1\"},"
                                + "{\"inventoryItemId\":\"" + cupId + "\",\"quantity\":\"2\"}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
        assertThat(jdbc.queryForObject("select count(*) from product_inventory_recipes where product_id = ?",
                Long.class, UUID.fromString(productId))).isZero();
    }

    @Test
    void allowsRecipeAdministrationForInactiveProductButRejectsNewInactiveInventoryItems() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = product(fx, token, "INACTIVEPROD");
        String url = recipeUrl(fx, productId);
        mockMvc.perform(patch(BASE + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        String itemId = item(fx, "inactive-" + UUID.randomUUID().toString().substring(0, 8), "GRAM", false);
        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(put(url).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + itemId + "\",\"quantity\":\"1\"}]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("INVENTORY_ITEM_INACTIVE"));
        assertThat(jdbc.queryForObject("select count(*) from product_inventory_recipes where product_id = ?",
                Long.class, UUID.fromString(productId))).isZero();
    }

    @Test
    void resolverAggregatesBaseAndSelectedModifierEffectsExactlyAndRemovesZeroTotals() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = product(fx, token, "RESOLVER");
        String coffeeId = item(fx, "resolver-coffee-" + UUID.randomUUID().toString().substring(0, 6), "GRAM", false);
        String milkId = item(fx, "resolver-milk-" + UUID.randomUUID().toString().substring(0, 6), "MILLILITER", false);
        String oatId = item(fx, "resolver-oat-" + UUID.randomUUID().toString().substring(0, 6), "MILLILITER", false);
        assertThat(resolver.resolve(fx.orgId(), UUID.fromString(productId), java.util.List.of()).tracked()).isFalse();

        mockMvc.perform(put(recipeUrl(fx, productId)).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + coffeeId + "\",\"quantity\":\"18\"},"
                                + "{\"inventoryItemId\":\"" + milkId + "\",\"quantity\":\"250\"}]}"))
                .andExpect(status().isOk());
        String oatOption = modifierOption(fx, token, "oat");
        String extraShot = modifierOption(fx, token, "shot");
        replaceEffects(fx, token, oatOption, "[{\"inventoryItemId\":\"" + milkId
                + "\",\"quantityDelta\":\"-250\"},{\"inventoryItemId\":\"" + oatId
                + "\",\"quantityDelta\":\"250\"}]");
        replaceEffects(fx, token, extraShot, "[{\"inventoryItemId\":\"" + coffeeId
                + "\",\"quantityDelta\":\"18.000000\"}]");

        var resolved = resolver.resolve(fx.orgId(), UUID.fromString(productId),
                java.util.List.of(UUID.fromString(oatOption), UUID.fromString(extraShot)));
        assertThat(resolved.tracked()).isTrue();
        assertThat(resolved.components()).extracting(component -> component.inventoryItemId().toString())
                .containsExactlyInAnyOrder(coffeeId, oatId);
        assertThat(resolved.components()).filteredOn(c -> c.inventoryItemId().toString().equals(coffeeId))
                .singleElement().extracting(c -> c.quantity()).isEqualTo(new java.math.BigDecimal("36.000000"));
        assertThat(resolved.components()).filteredOn(c -> c.inventoryItemId().toString().equals(oatId))
                .singleElement().extracting(c -> c.quantity()).isEqualTo(new java.math.BigDecimal("250.000000"));

        String invalidOption = modifierOption(fx, token, "negative");
        replaceEffects(fx, token, invalidOption, "[{\"inventoryItemId\":\"" + milkId
                + "\",\"quantityDelta\":\"-300\"}]");
        assertThatThrownBy(() -> resolver.resolve(fx.orgId(), UUID.fromString(productId),
                java.util.List.of(UUID.fromString(invalidOption))))
                .isInstanceOf(BusinessException.class)
                .extracting(exception -> ((BusinessException) exception).getCode())
                .isEqualTo("INVENTORY_CONFIGURATION_UNAVAILABLE");
    }

    @Test
    void resolverRejectsInactiveComponent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = product(fx, token, "RESOLVERUNIT");
        String unitId = item(fx, "resolver-unit-" + UUID.randomUUID().toString().substring(0, 6), "UNIT", false);
        mockMvc.perform(put(recipeUrl(fx, productId)).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + unitId + "\",\"quantity\":\"1\"}]}"))
                .andExpect(status().isOk());
        jdbc.update("update inventory_items set status = 'INACTIVE' where id = ?", UUID.fromString(unitId));
        assertThatThrownBy(() -> resolver.resolve(fx.orgId(), UUID.fromString(productId), java.util.List.of()))
                .isInstanceOf(BusinessException.class)
                .extracting(exception -> ((BusinessException) exception).getCode())
                .isEqualTo("INVENTORY_CONFIGURATION_UNAVAILABLE");
        mockMvc.perform(get(recipeUrl(fx, productId)).header("Authorization", bearer(token)))
                .andExpect(status().isOk()).andExpect(jsonPath("$.components[0].status").value("INACTIVE"));
        mockMvc.perform(put(recipeUrl(fx, productId)).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"INACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + unitId + "\",\"quantity\":\"1\"}]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.version").value(2));
        assertThat(resolver.resolve(fx.orgId(), UUID.fromString(productId), java.util.List.of()).tracked()).isFalse();
    }

    @Test
    void postgresEnforcesRecipeUniquenessPositiveQuantityAndSameOrganizationComponents() throws Exception {
        RbacFixture fx = fixture();
        RbacFixture other = fixture();
        String token = adminToken(fx);
        String productId = product(fx, token, "DBRECIPE");
        String itemId = item(fx, "db-recipe-" + UUID.randomUUID().toString().substring(0, 6), "GRAM", false);
        String otherItemId = item(other, "other-recipe-" + UUID.randomUUID().toString().substring(0, 6), "GRAM", false);
        mockMvc.perform(put(recipeUrl(fx, productId)).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + itemId + "\",\"quantity\":\"1\"}]}"))
                .andExpect(status().isOk());
        UUID recipeId = jdbc.queryForObject("select id from product_inventory_recipes where product_id = ?",
                UUID.class, UUID.fromString(productId));
        mockMvc.perform(put(recipeUrl(fx, productId)).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"ACTIVE\",\"components\":["
                                + "{\"inventoryItemId\":\"" + otherItemId + "\",\"quantity\":\"1\"}]}"))
                .andExpect(status().isNotFound()).andExpect(jsonPath("$.code").value("INVENTORY_ITEM_NOT_FOUND"));
        UUID organizationId = fx.orgId();
        assertThatThrownBy(() -> jdbc.update("insert into product_inventory_recipes "
                        + "(id,organization_id,product_id,status,version,created_at,updated_at) "
                        + "select ?,organization_id,product_id,'INACTIVE',0,now(),now() "
                        + "from product_inventory_recipes where id = ?", UUID.randomUUID(), recipeId))
                .isInstanceOf(org.springframework.dao.DataIntegrityViolationException.class);
        assertThatThrownBy(() -> jdbc.update("insert into product_inventory_recipe_components "
                        + "(id,organization_id,recipe_id,inventory_item_id,quantity_per_product) values (?,?,?,?,?)",
                        UUID.randomUUID(), organizationId, recipeId, UUID.fromString(itemId), new java.math.BigDecimal("2")))
                .isInstanceOf(org.springframework.dao.DataIntegrityViolationException.class);
        assertThatThrownBy(() -> jdbc.update("insert into product_inventory_recipe_components "
                        + "(id,organization_id,recipe_id,inventory_item_id,quantity_per_product) values (?,?,?,?,?)",
                        UUID.randomUUID(), organizationId, recipeId, UUID.randomUUID(), new java.math.BigDecimal("0")))
                .isInstanceOf(org.springframework.dao.DataIntegrityViolationException.class);
        assertThatThrownBy(() -> jdbc.update("insert into product_inventory_recipe_components "
                        + "(id,organization_id,recipe_id,inventory_item_id,quantity_per_product) values (?,?,?,?,?)",
                        UUID.randomUUID(), organizationId, recipeId, UUID.fromString(otherItemId), new java.math.BigDecimal("1")))
                .isInstanceOf(org.springframework.dao.DataIntegrityViolationException.class);
    }

    @Test
    void recipeRoutesRequireTheirOwnOrganizationPermissions() throws Exception {
        RbacFixture fx = fixture();
        String admin = adminToken(fx);
        String productId = product(fx, admin, "AUTHRECIPE");
        String url = recipeUrl(fx, productId);
        mockMvc.perform(put(url).header("Authorization", bearer(admin)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\",\"components\":[]}"))
                .andExpect(status().isOk());
        var noRoleUser = createUserForOrganization(fx.orgId().toString(), uniqueEmail("recipe-no-role@"), PASSWORD);
        RbacFixture noRoleFixture = new RbacFixture(fx.orgId(), fx.orgCode(), noRoleUser.getEmail(), noRoleUser.getId());
        String noRole = login(noRoleFixture);
        mockMvc.perform(get(url).header("Authorization", bearer(noRole))).andExpect(status().isForbidden());
        mockMvc.perform(put(url).header("Authorization", bearer(noRole)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\",\"components\":[]}"))
                .andExpect(status().isForbidden());
        var readerUser = createUserForOrganization(fx.orgId().toString(), uniqueEmail("recipe-reader@"), PASSWORD);
        RbacFixture readerFixture = new RbacFixture(fx.orgId(), fx.orgCode(), readerUser.getEmail(), readerUser.getId());
        createRoleAndAssign(readerFixture, "recipe-reader", "Recipe Reader", "inventory.recipe.read");
        String reader = login(readerFixture);
        mockMvc.perform(get(url).header("Authorization", bearer(reader))).andExpect(status().isOk());
        mockMvc.perform(put(url).header("Authorization", bearer(reader)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\",\"components\":[]}"))
                .andExpect(status().isForbidden());
    }
}
