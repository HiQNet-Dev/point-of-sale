package com.hiqnet.pos.inventory.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectChange;
import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectsService;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeComponentChange;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeService;
import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus;
import com.hiqnet.pos.inventory.stock.application.InventoryStockService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.purchasing.application.SupplierService;
import java.math.BigDecimal;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Guarda de inmutabilidad de la unidad base del InventoryItem sobre la cadena
 * de seguridad real (JWT + RBAC) y PostgreSQL real.
 *
 * <p>La unidad solo puede corregirse mientras el artículo no tenga hechos
 * dependientes; en caso contrario la API responde {@code 409
 * INVENTORY_ITEM_UNIT_LOCKED} y no modifica {@code base_unit}.
 */
class InventoryItemBaseUnitGuardApiTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired
    private InventoryItemService itemService;

    @Autowired
    private InventoryRecipeService recipes;

    @Autowired
    private ModifierInventoryEffectsService effects;

    @Autowired
    private InventoryStockService stock;

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private OrganizationRepository organizations;

    @Autowired
    private ProductRepository products;

    @Autowired
    private ModifierGroupRepository groups;

    @Autowired
    private ModifierOptionRepository options;

    private UUID createItem(RbacFixture fx, String unit, boolean fractional) {
        return itemService
                .create(fx.orgId(), "item-" + UUID.randomUUID().toString().substring(0, 8), "Item", null,
                        InventoryBaseUnit.valueOf(unit), fractional)
                .getId();
    }

    private int patchBaseUnit(RbacFixture fx, String token, UUID itemId, long version, String unit) throws Exception {
        return mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + version + ",\"baseUnit\":\"" + unit + "\"}"))
                .andReturn()
                .getResponse()
                .getStatus();
    }

    private String storedUnit(UUID itemId) {
        return jdbcTemplate.queryForObject(
                "select base_unit from inventory_items where id = ?", String.class, itemId);
    }

    private boolean storedLocked(UUID itemId) {
        Boolean locked = jdbcTemplate.queryForObject(
                "select unit_locked from inventory_items where id = ?", Boolean.class, itemId);
        return Boolean.TRUE.equals(locked);
    }

    @Test
    void unitChangeIsAllowedWhileNoDependenciesExistAndAudits() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID itemId = createItem(fx, "GRAM", false);
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"baseUnit\":\"MILLILITER\",\"fractionalUnitAllowed\":true}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.baseUnit").value("MILLILITER"))
                .andExpect(jsonPath("$.unitLocked").value(false))
                .andExpect(jsonPath("$.version").value(1));

        assertThat(storedUnit(itemId)).isEqualTo("MILLILITER");
        assertThat(storedLocked(itemId)).isFalse();
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("inventory.item.updated");
        assertThat(metadataOf(latestAudit(fx.orgId())).get("changedFields").toString())
                .isEqualTo("[baseUnit, fractionalUnitAllowed]");
    }

    @Test
    void sameUnitRequestIsIdempotentWithoutVersionOrAudit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID itemId = createItem(fx, "GRAM", false);
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Item\",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.baseUnit").value("GRAM"))
                .andExpect(jsonPath("$.version").value(0));
        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }

    @Test
    void unitChangeIsRejectedAfterARecipeComponentExists() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizations.findById(fx.orgId()).orElseThrow();
        UUID itemId = createItem(fx, "GRAM", false);
        UUID productId = products.saveAndFlush(Product.create(
                        organization, "SKU-" + UUID.randomUUID().toString().substring(0, 6).toUpperCase(Locale.ROOT),
                        "Product", BigDecimal.TEN))
                .getId();
        recipes.replace(fx.orgId(), productId, 0, InventoryRecipeStatus.ACTIVE,
                List.of(new InventoryRecipeComponentChange(itemId, new BigDecimal("2"))));

        assertThat(patchBaseUnit(fx, token, itemId, 0, "UNIT")).isEqualTo(409);
        assertThat(storedUnit(itemId)).isEqualTo("GRAM");
        assertThat(storedLocked(itemId)).isTrue();
    }

    @Test
    void unitChangeIsRejectedAfterAModifierEffectExists() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizations.findById(fx.orgId()).orElseThrow();
        UUID itemId = createItem(fx, "MILLILITER", false);
        String suffix = UUID.randomUUID().toString().substring(0, 8);
        ModifierGroup group = groups.saveAndFlush(
                ModifierGroup.create(organization, "mg-" + suffix, "Group", 0, 1));
        ModifierOption option = options.saveAndFlush(ModifierOption.create(
                organization, group, "mo-" + suffix, "Option", BigDecimal.ZERO, 0));
        effects.replace(fx.orgId(), option.getId(), 0,
                List.of(new ModifierInventoryEffectChange(itemId, new BigDecimal("-50"))));

        assertThat(patchBaseUnit(fx, token, itemId, 0, "UNIT")).isEqualTo(409);
        assertThat(storedUnit(itemId)).isEqualTo("MILLILITER");
        assertThat(storedLocked(itemId)).isTrue();
    }

    @Test
    void unitChangeIsRejectedAfterAStockMovementExists() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID itemId = createItem(fx, "GRAM", false);
        UUID branchId = branchService.create(fx.orgId(), "Centro", "sucursal-centro", null).getId();
        stock.open(fx.orgId(), branchId, itemId, fx.userId(), 0, "10", null);

        assertThat(patchBaseUnit(fx, token, itemId, 0, "UNIT")).isEqualTo(409);
        assertThat(storedUnit(itemId)).isEqualTo("GRAM");
        assertThat(storedLocked(itemId)).isTrue();
    }

    @Test
    void unitChangeIsRejectedAfterADraftPurchaseOrderLineExists() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID itemId = createItem(fx, "UNIT", false);
        String orderId = draftOrderWithLine(fx, token, itemId);

        assertThat(orderId).isNotBlank();
        assertThat(patchBaseUnit(fx, token, itemId, 0, "GRAM")).isEqualTo(409);
        assertThat(storedUnit(itemId)).isEqualTo("UNIT");
        assertThat(storedLocked(itemId)).isTrue();
    }

    @Test
    void unitChangeIsRejectedAfterAPurchaseOrderIsIssued() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID itemId = createItem(fx, "UNIT", false);
        String orderId = draftOrderWithLine(fx, token, itemId);
        mockMvc.perform(post(ordersUrl(fx, orderId) + "/issue")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("ORDERED"));

        assertThat(patchBaseUnit(fx, token, itemId, 0, "GRAM")).isEqualTo(409);
        assertThat(storedUnit(itemId)).isEqualTo("UNIT");
    }

    @Test
    void unitStaysRejectedAfterTheDependencyIsRemoved() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizations.findById(fx.orgId()).orElseThrow();
        UUID itemId = createItem(fx, "GRAM", true);
        UUID productId = products.saveAndFlush(Product.create(
                        organization, "SKU-" + UUID.randomUUID().toString().substring(0, 6).toUpperCase(Locale.ROOT),
                        "Product", BigDecimal.TEN))
                .getId();
        recipes.replace(fx.orgId(), productId, 0, InventoryRecipeStatus.ACTIVE,
                List.of(new InventoryRecipeComponentChange(itemId, new BigDecimal("2"))));
        // La receta pasa a INACTIVE y sin componentes: la evidencia histórica
        // permanece y la unidad sigue congelada.
        recipes.replace(fx.orgId(), productId, 1, InventoryRecipeStatus.INACTIVE, List.of());

        assertThat(patchBaseUnit(fx, token, itemId, 0, "UNIT")).isEqualTo(409);
        assertThat(storedLocked(itemId)).isTrue();
    }

    @Test
    void lockedItemStillAllowsNonUnitUpdates() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID itemId = createItem(fx, "GRAM", false);
        UUID branchId = branchService.create(fx.orgId(), "Centro", "sucursal-centro", null).getId();
        stock.open(fx.orgId(), branchId, itemId, fx.userId(), 0, "10", null);

        mockMvc.perform(patch(BASE + fx.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Renamed\",\"status\":\"INACTIVE\",\"fractionalUnitAllowed\":true}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Renamed"))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.baseUnit").value("GRAM"))
                .andExpect(jsonPath("$.unitLocked").value(true));
        assertThat(storedUnit(itemId)).isEqualTo("GRAM");
    }

    @Test
    void unitLockDoesNotLeakAcrossOrganizations() throws Exception {
        RbacFixture locked = fixture();
        RbacFixture other = fixture();
        String otherToken = adminToken(other);
        UUID lockedItem = createItem(locked, "GRAM", false);
        UUID branchId = branchService.create(locked.orgId(), "Centro", "sucursal-centro", null).getId();
        stock.open(locked.orgId(), branchId, lockedItem, locked.userId(), 0, "5", null);
        UUID otherItem = createItem(other, "GRAM", false);

        assertThat(patchBaseUnit(locked, adminToken(locked), lockedItem, 0, "UNIT")).isEqualTo(409);
        // El artículo de la otra organización sigue libre.
        assertThat(patchBaseUnit(other, otherToken, otherItem, 0, "UNIT")).isEqualTo(200);
        assertThat(storedUnit(otherItem)).isEqualTo("UNIT");
    }

    private String draftOrderWithLine(RbacFixture fx, String token, UUID itemId) throws Exception {
        UUID branchId = branchService.create(fx.orgId(), "Centro", "sucursal-centro", null).getId();
        UUID supplierId = supplierService
                .create(fx.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null)
                .getId();
        String ordersUrl = BASE + fx.orgId() + "/branches/" + branchId + "/purchase-orders";
        MvcResult created = mockMvc.perform(post(ordersUrl)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-guard-" + UUID.randomUUID().toString().substring(0, 6)
                                + "\",\"supplierId\":\"" + supplierId + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        String orderId = objectMapper.readTree(created.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();
        mockMvc.perform(post(ordersUrl + "/" + orderId + "/lines")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + itemId + "\",\"quantity\":\"1.000000\"}"))
                .andExpect(status().isCreated());
        return orderId;
    }

    private String ordersUrl(RbacFixture fx, String orderId) {
        UUID branchId = jdbcTemplate.queryForObject(
                "select branch_id from purchase_orders where id = ?", UUID.class, UUID.fromString(orderId));
        return BASE + fx.orgId() + "/branches/" + branchId + "/purchase-orders/" + orderId;
    }
}
