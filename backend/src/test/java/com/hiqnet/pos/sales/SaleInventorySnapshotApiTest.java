package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.organization.domain.Organization;
import java.math.BigDecimal;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;

class SaleInventorySnapshotApiTest extends AbstractRbacIntegrationTest {
    private static final String ORG = "/api/v1/organizations/";

    @Autowired private BranchRepository branches;
    @Autowired private CashRegisterRepository registers;
    @Autowired private CashShiftRepository shifts;
    @Autowired private ProductRepository products;
    @Autowired private JdbcTemplate jdbc;

    @Test
    void snapshotsResolvedComponentsAndRefreshesOnlyOnMaterialRequote() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, "snapshot-main");
        String productId = product(fx, token, "LATTE", "Latte", "10.0000");
        String coffeeId = item(fx, token, "coffee", "Coffee", "GRAM", false);
        String milkId = item(fx, token, "whole-milk", "Whole Milk", "MILLILITER", false);
        String oatId = item(fx, token, "oat-milk", "Oat Milk", "MILLILITER", false);
        String cupId = item(fx, token, "cup", "Cup", "UNIT", false);
        String groupId = modifierGroup(fx, token, "milk-and-shot");
        String oatOption = modifierOption(fx, token, groupId, "oat", "Oat", "1.0000");
        String shotOption = modifierOption(fx, token, groupId, "shot", "Extra shot", "2.0000");
        assignGroup(fx, token, productId, groupId);

        replaceRecipe(fx, token, productId, 0, "ACTIVE", List.of(
                component(coffeeId, "18"), component(milkId, "250"), component(cupId, "1")));
        replaceEffects(fx, token, oatOption, 0, List.of(effect(milkId, "-250"), effect(oatId, "250")));
        replaceEffects(fx, token, shotOption, 0, List.of(effect(coffeeId, "18")));

        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, productId, 2, List.of(oatOption, shotOption))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.version").value(1));
        String saleItemId = jdbc.queryForObject("select id from sale_items where sale_id = ?", UUID.class,
                UUID.fromString(saleId)).toString();
        assertThat(snapshot(saleItemId)).hasSize(3);
        assertSnapshot(saleItemId, coffeeId, "coffee", "Coffee", "GRAM", "36", "72");
        assertSnapshot(saleItemId, oatId, "oat-milk", "Oat Milk", "MILLILITER", "250", "500");
        assertSnapshot(saleItemId, cupId, "cup", "Cup", "UNIT", "1", "2");
        assertThat(snapshotRow(saleItemId, milkId)).isNull();
        assertThat(auditCount(fx.orgId(), "sale.item.added")).isEqualTo(1);
        List<Map<String, Object>> originalSnapshot = snapshot(saleItemId);

        mockMvc.perform(patch(ORG + fx.orgId() + "/inventory-items/" + coffeeId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Coffee renamed\"}"))
                .andExpect(status().isOk());
        replaceRecipe(fx, token, productId, 1, "ACTIVE", List.of(
                component(coffeeId, "20"), component(milkId, "250"), component(cupId, "1")));
        replaceEffects(fx, token, shotOption, 1, List.of(effect(coffeeId, "5")));

        assertSnapshot(saleItemId, coffeeId, "coffee", "Coffee", "GRAM", "36", "72");
        assertSnapshot(saleItemId, oatId, "oat-milk", "Oat Milk", "MILLILITER", "250", "500");
        assertThat(snapshot(saleItemId)).containsExactlyElementsOf(originalSnapshot);
        assertThat(auditCount(fx.orgId(), "sale.item.added")).isEqualTo(1);
        assertThat(auditCount(fx.orgId(), "sale.item.updated")).isZero();

        addItemUpdate(scope, token, saleId, saleItemId, 1, 3, List.of(shotOption))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.version").value(2));
        assertThat(snapshot(saleItemId)).hasSize(3);
        assertSnapshot(saleItemId, coffeeId, "coffee", "Coffee renamed", "GRAM", "25", "75");
        assertSnapshot(saleItemId, milkId, "whole-milk", "Whole Milk", "MILLILITER", "250", "750");
        assertSnapshot(saleItemId, cupId, "cup", "Cup", "UNIT", "1", "3");
        assertThat(snapshotRow(saleItemId, oatId)).isNull();
        assertThat(auditCount(fx.orgId(), "sale.item.updated")).isEqualTo(1);

        List<Map<String, Object>> beforeNoOp = snapshot(saleItemId);
        long auditsBeforeNoOp = auditCount(fx.orgId());
        addItemUpdate(scope, token, saleId, saleItemId, 2, 3, List.of(shotOption))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.version").value(2));
        assertThat(snapshot(saleItemId)).containsExactlyElementsOf(beforeNoOp);
        assertThat(auditCount(fx.orgId())).isEqualTo(auditsBeforeNoOp);

        replaceRecipe(fx, token, productId, 2, "INACTIVE", List.of(
                component(coffeeId, "20"), component(milkId, "250"), component(cupId, "1")));
        mockMvc.perform(patch(ORG + fx.orgId() + "/inventory-items/" + coffeeId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        assertThat(snapshot(saleItemId)).containsExactlyElementsOf(beforeNoOp);

        mockMvc.perform(post(saleUrl(scope) + "/" + saleId + "/items/" + saleItemId + "/remove")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.items[0].status").value("REMOVED"));
        assertThat(snapshot(saleItemId)).containsExactlyElementsOf(beforeNoOp);
        mockMvc.perform(post(saleUrl(scope) + "/" + saleId + "/cancel")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":3}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("CANCELLED"));
        assertThat(snapshot(saleItemId)).containsExactlyElementsOf(beforeNoOp);
    }

    @Test
    void untrackedLinesRemainValidAndBrokenTrackedConfigurationRollsBackTheSaleMutation() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, "snapshot-invalid");
        String untrackedProduct = product(fx, token, "UNTRACKED", "Untracked", "1.0000");
        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, untrackedProduct, 1, List.of()).andExpect(status().isOk());
        UUID untrackedItemId = jdbc.queryForObject("select id from sale_items where sale_id = ?", UUID.class,
                UUID.fromString(saleId));
        assertThat(snapshot(untrackedItemId.toString())).isEmpty();

        String trackedProduct = product(fx, token, "TRACKED", "Tracked", "1.0000");
        String flourId = item(fx, token, "flour", "Flour", "GRAM", false);
        replaceRecipe(fx, token, trackedProduct, 0, "ACTIVE", List.of(component(flourId, "50")));
        String groupId = modifierGroup(fx, token, "tracked-options");
        String optionId = modifierOption(fx, token, groupId, "tracked-option", "Tracked option", "0.0000");
        assignGroup(fx, token, trackedProduct, groupId);
        String existingSale = sale(scope, token);
        addItem(scope, token, existingSale, 0, trackedProduct, 1, List.of(optionId)).andExpect(status().isOk());
        UUID existingSaleItemId = jdbc.queryForObject("select id from sale_items where sale_id = ?", UUID.class,
                UUID.fromString(existingSale));
        mockMvc.perform(patch(ORG + fx.orgId() + "/inventory-items/" + flourId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        String failedSale = sale(scope, token);
        long auditsBefore = auditCount(fx.orgId(), "sale.item.added");
        addItem(scope, token, failedSale, 0, trackedProduct, 1, List.of(optionId))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INVENTORY_CONFIGURATION_UNAVAILABLE"));
        assertThat(jdbc.queryForObject("select version from sales where id = ?", Long.class,
                UUID.fromString(failedSale))).isZero();
        assertThat(jdbc.queryForObject("select count(*) from sale_items where sale_id = ?", Long.class,
                UUID.fromString(failedSale))).isZero();
        assertThat(auditCount(fx.orgId(), "sale.item.added")).isEqualTo(auditsBefore);

        addItemUpdate(scope, token, existingSale, existingSaleItemId.toString(), 1, 2, List.of(optionId))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INVENTORY_CONFIGURATION_UNAVAILABLE"));
        assertThat(jdbc.queryForObject("select version from sales where id = ?", Long.class,
                UUID.fromString(existingSale))).isEqualTo(1L);
        assertThat(jdbc.queryForObject("select quantity from sale_items where id = ?", Integer.class,
                existingSaleItemId)).isEqualTo(1);
        assertThat(jdbc.queryForObject("select count(*) from sale_item_modifiers where sale_item_id = ?", Long.class,
                existingSaleItemId)).isEqualTo(1L);
        assertThat(jdbc.queryForObject("select modifier_option_id from sale_item_modifiers where sale_item_id = ?", UUID.class,
                existingSaleItemId)).isEqualTo(UUID.fromString(optionId));
        assertSnapshot(existingSaleItemId.toString(), flourId, "flour", "Flour", "GRAM", "50", "50");

        String inactiveRecipeProduct = product(fx, token, "INACTIVERECIPE", "Inactive recipe", "1.0000");
        replaceRecipe(fx, token, inactiveRecipeProduct, 0, "INACTIVE", List.of());
        String inactiveRecipeSale = sale(scope, token);
        addItem(scope, token, inactiveRecipeSale, 0, inactiveRecipeProduct, 1, List.of()).andExpect(status().isOk());
        UUID inactiveRecipeSaleItem = jdbc.queryForObject("select id from sale_items where sale_id = ?", UUID.class,
                UUID.fromString(inactiveRecipeSale));
        assertThat(snapshot(inactiveRecipeSaleItem.toString())).isEmpty();
    }

    @Test
    void quantityOnlyRequoteUsesCurrentRecipeAndLegacyLineReceivesSnapshotOnEdit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, "snapshot-legacy-requote");
        String productId = product(fx, token, "REQUOTE", "Requote", "1.0000");
        String beansId = item(fx, token, "requote-beans", "Requote beans", "GRAM", false);
        replaceRecipe(fx, token, productId, 0, "ACTIVE", List.of(component(beansId, "18")));

        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, productId, 1, List.of()).andExpect(status().isOk());
        String saleItemId = jdbc.queryForObject("select id from sale_items where sale_id = ?", UUID.class,
                UUID.fromString(saleId)).toString();
        assertSnapshot(saleItemId, beansId, "requote-beans", "Requote beans", "GRAM", "18", "18");

        replaceRecipe(fx, token, productId, 1, "ACTIVE", List.of(component(beansId, "20")));
        addItemUpdate(scope, token, saleId, saleItemId, 1, 3, List.of()).andExpect(status().isOk());
        assertThat(jdbc.queryForObject("select version from sales where id = ?", Long.class, UUID.fromString(saleId)))
                .isEqualTo(2L);
        assertSnapshot(saleItemId, beansId, "requote-beans", "Requote beans", "GRAM", "20", "60");

        String legacySaleId = sale(scope, token);
        addItem(scope, token, legacySaleId, 0, productId, 1, List.of()).andExpect(status().isOk());
        String legacyItemId = jdbc.queryForObject("select id from sale_items where sale_id = ?", UUID.class,
                UUID.fromString(legacySaleId)).toString();
        jdbc.update("delete from sale_item_inventory_components where sale_item_id = ?", UUID.fromString(legacyItemId));
        assertThat(snapshot(legacyItemId)).isEmpty();

        addItemUpdate(scope, token, legacySaleId, legacyItemId, 1, 2, List.of()).andExpect(status().isOk());
        assertThat(jdbc.queryForObject("select version from sales where id = ?", Long.class, UUID.fromString(legacySaleId)))
                .isEqualTo(2L);
        assertSnapshot(legacyItemId, beansId, "requote-beans", "Requote beans", "GRAM", "20", "40");
    }

    @Test
    void totalQuantityOverflowIsRejectedBeforeSaleMutation() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, "snapshot-overflow");
        String productId = product(fx, token, "OVERFLOW", "Overflow", "1.0000");
        String itemId = item(fx, token, "maximum-item", "Maximum item", "GRAM", false);
        replaceRecipe(fx, token, productId, 0, "ACTIVE", List.of(component(itemId, "9999999999999.999999")));
        String saleId = sale(scope, token);

        addItem(scope, token, saleId, 0, productId, 2, List.of())
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_INVENTORY_QUANTITY"));
        assertThat(jdbc.queryForObject("select version from sales where id = ?", Long.class,
                UUID.fromString(saleId))).isZero();
        assertThat(jdbc.queryForObject("select count(*) from sale_items where sale_id = ?", Long.class,
                UUID.fromString(saleId))).isZero();
    }

    @Test
    void snapshotDatabaseEnforcesOrganizationScopeAndOneComponentPerItem() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, "snapshot-constraints");
        String productId = product(fx, token, "CONSTRAINTS", "Constraints", "1.0000");
        String itemId = item(fx, token, "constraint-item", "Constraint item", "GRAM", false);
        replaceRecipe(fx, token, productId, 0, "ACTIVE", List.of(component(itemId, "4")));
        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, productId, 1, List.of()).andExpect(status().isOk());
        UUID saleItemId = jdbc.queryForObject("select id from sale_items where sale_id = ?", UUID.class,
                UUID.fromString(saleId));

        String insert = "insert into sale_item_inventory_components (id,organization_id,sale_item_id,inventory_item_id,"
                + "inventory_item_code_snapshot,inventory_item_name_snapshot,base_unit_snapshot,quantity_per_unit,quantity_total,created_at) "
                + "values (?,?,?, ?, 'foreign-code','Foreign item','GRAM',1,1,now())";
        assertThatThrownBy(() -> jdbc.update(insert, UUID.randomUUID(), fx.orgId(), saleItemId, UUID.fromString(itemId)))
                .isInstanceOf(DataIntegrityViolationException.class);

        RbacFixture foreignFx = fixture();
        String foreignToken = adminToken(foreignFx);
        String foreignItemId = item(foreignFx, foreignToken, "foreign-item", "Foreign item", "GRAM", false);
        assertThatThrownBy(() -> jdbc.update(insert, UUID.randomUUID(), fx.orgId(), saleItemId,
                UUID.fromString(foreignItemId))).isInstanceOf(DataIntegrityViolationException.class);

        String unitItemId = item(fx, token, "invalid-unit-item", "Invalid unit item", "GRAM", false);
        String zeroItemId = item(fx, token, "zero-quantity-item", "Zero quantity item", "GRAM", false);
        String negativeItemId = item(fx, token, "negative-quantity-item", "Negative quantity item", "GRAM", false);
        String invalidUnit = "insert into sale_item_inventory_components (id,organization_id,sale_item_id,inventory_item_id,"
                + "inventory_item_code_snapshot,inventory_item_name_snapshot,base_unit_snapshot,quantity_per_unit,quantity_total,created_at) "
                + "values (?,?,?,?,'bad','Bad','NOT_A_UNIT',1,1,now())";
        assertThatThrownBy(() -> jdbc.update(invalidUnit, UUID.randomUUID(), fx.orgId(), saleItemId,
                UUID.fromString(unitItemId))).isInstanceOf(DataIntegrityViolationException.class);
        String zeroQuantity = "insert into sale_item_inventory_components (id,organization_id,sale_item_id,inventory_item_id,"
                + "inventory_item_code_snapshot,inventory_item_name_snapshot,base_unit_snapshot,quantity_per_unit,quantity_total,created_at) "
                + "values (?,?,?,?,'zero','Zero','GRAM',0,1,now())";
        assertThatThrownBy(() -> jdbc.update(zeroQuantity, UUID.randomUUID(), fx.orgId(), saleItemId,
                UUID.fromString(zeroItemId))).isInstanceOf(DataIntegrityViolationException.class);
        String negativeQuantity = "insert into sale_item_inventory_components (id,organization_id,sale_item_id,inventory_item_id,"
                + "inventory_item_code_snapshot,inventory_item_name_snapshot,base_unit_snapshot,quantity_per_unit,quantity_total,created_at) "
                + "values (?,?,?,?,'negative','Negative','GRAM',1,-1,now())";
        assertThatThrownBy(() -> jdbc.update(negativeQuantity, UUID.randomUUID(), fx.orgId(), saleItemId,
                UUID.fromString(negativeItemId))).isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void cashierWithOnlySalesPermissionsCanAddTrackedItemsAndCheckoutDoesNotMutateStock() throws Exception {
        RbacFixture fx = fixture();
        String admin = adminToken(fx);
        Scope scope = scope(fx, "snapshot-cashier");
        String productId = product(fx, admin, "CASHIER", "Cashier product", "0.0000");
        String beansId = item(fx, admin, "beans", "Beans", "GRAM", false);
        replaceRecipe(fx, admin, productId, 0, "ACTIVE", List.of(component(beansId, "12.5")));

        var cashierUser = createUserForOrganization(fx.orgId().toString(), uniqueEmail("snapshot-cashier@"), PASSWORD);
        RbacFixture cashierFx = new RbacFixture(fx.orgId(), fx.orgCode(), cashierUser.getEmail(), cashierUser.getId());
        createRoleAndAssign(cashierFx, "snapshot-sales-only", "Snapshot Sales Only", "sale.create", "sale.update", "sale.checkout");
        String cashier = login(cashierFx);
        String saleId = sale(scope, cashier);
        addItem(scope, cashier, saleId, 0, productId, 2, List.of()).andExpect(status().isOk());
        String saleItemId = jdbc.queryForObject("select id from sale_items where sale_id = ?", String.class,
                UUID.fromString(saleId));
        assertSnapshot(saleItemId, beansId, "beans", "Beans", "GRAM", "12.5", "25");

        jdbc.update("insert into branch_inventory (id,organization_id,branch_id,inventory_item_id,quantity_on_hand,created_at,updated_at) "
                        + "values (?,?,?,?,?,now(),now())",
                UUID.randomUUID(), fx.orgId(), scope.branchId(), UUID.fromString(beansId), new BigDecimal("100"));
        long movementsBefore = jdbc.queryForObject("select count(*) from inventory_movements", Long.class);
        long balancesBefore = jdbc.queryForObject("select count(*) from branch_inventory", Long.class);
        mockMvc.perform(post(saleUrl(scope) + "/" + saleId + "/checkout")
                        .header("Authorization", bearer(cashier)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"payments\":[]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("COMPLETED"));
        assertThat(snapshot(saleItemId)).hasSize(1);
        assertThat(jdbc.queryForObject("select count(*) from inventory_movements", Long.class)).isEqualTo(movementsBefore);
        assertThat(jdbc.queryForObject("select count(*) from branch_inventory", Long.class)).isEqualTo(balancesBefore);
        assertThat(jdbc.queryForObject("select quantity_on_hand from branch_inventory where branch_id = ? and inventory_item_id = ?",
                BigDecimal.class, scope.branchId(), UUID.fromString(beansId))).isEqualByComparingTo("100");
    }

    private String item(RbacFixture fx, String token, String code, String name, String unit, boolean fractional)
            throws Exception {
        String body = "{\"code\":\"" + code + "\",\"name\":\"" + name
                + "\",\"baseUnit\":\"" + unit + "\",\"fractionalUnitAllowed\":" + fractional + "}";
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/inventory-items")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private String product(RbacFixture fx, String token, String sku, String name, String price) throws Exception {
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/products")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"sku\":\"" + sku + "\",\"name\":\"" + name + "\",\"unitPrice\":" + price + "}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private String modifierGroup(RbacFixture fx, String token, String code) throws Exception {
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/modifier-groups")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"code\":\"" + code + "\",\"name\":\"" + code + "\",\"minSelections\":0,\"maxSelections\":2}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private String modifierOption(RbacFixture fx, String token, String groupId, String code, String name, String price)
            throws Exception {
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/modifier-groups/" + groupId + "/options")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"code\":\"" + code + "\",\"name\":\"" + name
                        + "\",\"priceDelta\":" + price + ",\"displayOrder\":0}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private void assignGroup(RbacFixture fx, String token, String productId, String groupId) throws Exception {
        mockMvc.perform(put(ORG + fx.orgId() + "/products/" + productId + "/modifier-groups")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + groupId + "\"]}"))
                .andExpect(status().isOk());
    }

    private void replaceRecipe(RbacFixture fx, String token, String productId, int version, String state,
            List<String> components) throws Exception {
        String body = "{\"expectedVersion\":" + version + ",\"status\":\"" + state
                + "\",\"components\":[" + String.join(",", components) + "]}";
        mockMvc.perform(put(ORG + fx.orgId() + "/products/" + productId + "/inventory-recipe")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isOk());
    }

    private void replaceEffects(RbacFixture fx, String token, String optionId, int version, List<String> effects)
            throws Exception {
        String body = "{\"expectedVersion\":" + version + ",\"effects\":[" + String.join(",", effects) + "]}";
        mockMvc.perform(put(ORG + fx.orgId() + "/modifier-options/" + optionId + "/inventory-effects")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isOk());
    }

    private static String component(String itemId, String quantity) {
        return "{\"inventoryItemId\":\"" + itemId + "\",\"quantity\":\"" + quantity + "\"}";
    }

    private static String effect(String itemId, String delta) {
        return "{\"inventoryItemId\":\"" + itemId + "\",\"quantityDelta\":\"" + delta + "\"}";
    }

    private org.springframework.test.web.servlet.ResultActions addItem(Scope scope, String token, String saleId,
            long saleVersion, String productId, int quantity, List<String> options) throws Exception {
        String ids = options.stream().map(id -> "\"" + id + "\"").collect(java.util.stream.Collectors.joining(","));
        return mockMvc.perform(post(saleUrl(scope) + "/" + saleId + "/items")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"expectedSaleVersion\":" + saleVersion + ",\"productId\":\"" + productId
                        + "\",\"quantity\":" + quantity + ",\"modifierOptionIds\":[" + ids + "]}"));
    }

    private org.springframework.test.web.servlet.ResultActions addItemUpdate(Scope scope, String token, String saleId,
            String saleItemId, long saleVersion, int quantity, List<String> options) throws Exception {
        String ids = options.stream().map(id -> "\"" + id + "\"").collect(java.util.stream.Collectors.joining(","));
        return mockMvc.perform(patch(saleUrl(scope) + "/" + saleId + "/items/" + saleItemId)
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"expectedSaleVersion\":" + saleVersion + ",\"quantity\":" + quantity
                        + ",\"modifierOptionIds\":[" + ids + "]}"));
    }

    private Scope scope(RbacFixture fx, String branchCode) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch = branches.saveAndFlush(Branch.create(organization, "Branch " + branchCode, branchCode, null));
        CashRegister register = registers.saveAndFlush(CashRegister.create(organization, branch, "register", "Register"));
        CashShift shift = shifts.saveAndFlush(CashShift.open(fx.orgId(), branch.getId(), register.getId(), fx.userId(),
                BigDecimal.ZERO, java.time.Instant.now()));
        return new Scope(fx, branch.getId(), register.getId(), shift.getId());
    }

    private String sale(Scope scope, String token) throws Exception {
        return id(mockMvc.perform(post(saleUrl(scope)).header("Authorization", bearer(token)))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private static String saleUrl(Scope scope) {
        return ORG + scope.fixture().orgId() + "/branches/" + scope.branchId() + "/cash-registers/"
                + scope.registerId() + "/cash-shifts/" + scope.shiftId() + "/sales";
    }

    private String id(String json) throws Exception {
        return objectMapper.readTree(json).get("id").asText();
    }

    private List<Map<String, Object>> snapshot(String saleItemId) {
        return jdbc.queryForList("select id, inventory_item_id, inventory_item_code_snapshot, inventory_item_name_snapshot, "
                + "base_unit_snapshot, quantity_per_unit, quantity_total from sale_item_inventory_components "
                + "where sale_item_id = ? order by inventory_item_id", UUID.fromString(saleItemId));
    }

    private Map<String, Object> snapshotRow(String saleItemId, String inventoryItemId) {
        List<Map<String, Object>> rows = jdbc.queryForList("select id, inventory_item_id, inventory_item_code_snapshot, "
                + "inventory_item_name_snapshot, base_unit_snapshot, quantity_per_unit, quantity_total "
                + "from sale_item_inventory_components where sale_item_id = ? and inventory_item_id = ?",
                UUID.fromString(saleItemId), UUID.fromString(inventoryItemId));
        return rows.isEmpty() ? null : rows.getFirst();
    }

    private void assertSnapshot(String saleItemId, String inventoryItemId, String code, String name, String unit,
            String perUnit, String total) {
        Map<String, Object> row = snapshotRow(saleItemId, inventoryItemId);
        assertThat(row).isNotNull();
        assertThat(row.get("inventory_item_code_snapshot")).isEqualTo(code);
        assertThat(row.get("inventory_item_name_snapshot")).isEqualTo(name);
        assertThat(row.get("base_unit_snapshot")).isEqualTo(unit);
        assertThat((BigDecimal) row.get("quantity_per_unit")).isEqualByComparingTo(perUnit);
        assertThat((BigDecimal) row.get("quantity_total")).isEqualByComparingTo(total);
    }

    private long auditCount(UUID organizationId, String action) {
        return jdbc.queryForObject("select count(*) from audit_events where organization_id = ? and action = ?",
                Long.class, organizationId, action);
    }

    private long auditCount(UUID organizationId) {
        return jdbc.queryForObject("select count(*) from audit_events where organization_id = ?", Long.class, organizationId);
    }

    private record Scope(RbacFixture fixture, UUID branchId, UUID registerId, UUID shiftId) {}
}
