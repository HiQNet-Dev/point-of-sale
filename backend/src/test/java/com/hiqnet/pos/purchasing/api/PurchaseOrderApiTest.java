package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import java.math.BigDecimal;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Pruebas de integración de la API administrativa de órdenes de compra sobre la
 * cadena de seguridad real (JWT + RBAC en PostgreSQL) y PostgreSQL real.
 */
class PurchaseOrderApiTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private InventoryItemService inventoryItemService;

    private record Setup(RbacFixture fixture, String token, UUID branchId, UUID supplierId, UUID itemId) {
    }

    private Setup setup() throws Exception {
        RbacFixture fixture = fixture();
        return setup(fixture, adminToken(fixture));
    }

    private Setup setup(RbacFixture fixture, String token) {
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        InventoryItem item = inventoryItemService.create(
                fixture.orgId(), "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
        return new Setup(fixture, token, branch.getId(), supplier.getId(), item.getId());
    }

    private String ordersUrl(Setup setup) {
        return BASE + setup.fixture().orgId() + "/branches/" + setup.branchId() + "/purchase-orders";
    }

    private String createOrder(Setup setup, String code) throws Exception {
        MvcResult result = mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        return objectMapper.readTree(result.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();
    }

    private String addLine(Setup setup, String orderId, String itemId, String quantity, String unitCost, long expectedVersion)
            throws Exception {
        String unitCostField = unitCost == null ? "" : ",\"unitCost\":\"" + unitCost + "\"";
        MvcResult result = mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + expectedVersion + ",\"inventoryItemId\":\"" + itemId
                                + "\",\"quantity\":\"" + quantity + "\"" + unitCostField + "}"))
                .andExpect(status().isCreated())
                .andReturn();
        return objectMapper.readTree(result.getResponse().getContentAsString())
                .get("lines")
                .get(0)
                .get("id")
                .asText();
    }

    @Test
    void purchaseOrderRoutesRequireAuthentication() throws Exception {
        Setup setup = setup();
        mockMvc.perform(get(ordersUrl(setup))).andExpect(status().isUnauthorized());
        mockMvc.perform(get(ordersUrl(setup) + "/" + UUID.randomUUID())).andExpect(status().isUnauthorized());
    }

    @Test
    void createReturnsDraftWithServerOwnedCurrencyStatusAndSupplierSnapshots() throws Exception {
        Setup setup = setup();
        mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-2026-0001\",\"supplierId\":\"" + setup.supplierId()
                                + "\",\"currency\":\"USD\",\"status\":\"ORDERED\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.purchaseOrder.status").value("DRAFT"))
                .andExpect(jsonPath("$.purchaseOrder.currency").value("MXN"))
                .andExpect(jsonPath("$.purchaseOrder.code").value("po-2026-0001"))
                .andExpect(jsonPath("$.purchaseOrder.branchId").value(setup.branchId().toString()))
                .andExpect(jsonPath("$.purchaseOrder.supplierCodeSnapshot").value("proveedor-centro"))
                .andExpect(jsonPath("$.purchaseOrder.supplierNameSnapshot").value("Proveedor Centro"))
                .andExpect(jsonPath("$.purchaseOrder.version").value(0))
                .andExpect(jsonPath("$.purchaseOrder.createdByUserId").value(setup.fixture().userId().toString()))
                .andExpect(jsonPath("$.purchaseOrder.issuedAt").doesNotExist())
                .andExpect(jsonPath("$.lines").isEmpty());

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.created");
        assertThat(metadataOf(latestAudit(setup.fixture().orgId()))).containsOnlyKeys("code");
        assertThat(latestAudit(setup.fixture().orgId()).branchId()).isEqualTo(setup.branchId());
    }

    @Test
    void createValidatesInputAndEnforcesOrganizationScopedCodeUniqueness() throws Exception {
        Setup setupA = setup();
        Setup setupB = setup();
        String url = ordersUrl(setupA);

        mockMvc.perform(post(url)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"PO-2026\",\"supplierId\":\"" + setupA.supplierId() + "\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));

        mockMvc.perform(post(url)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-ajeno\",\"supplierId\":\"" + setupB.supplierId() + "\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("SUPPLIER_NOT_FOUND"));

        createOrder(setupA, "po-2026-0001");
        mockMvc.perform(post(url)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-2026-0001\",\"supplierId\":\"" + setupA.supplierId() + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_CODE_ALREADY_EXISTS"));

        createOrder(setupB, "po-2026-0001");
    }

    @Test
    void createRejectsInactiveSupplierAndInactiveBranch() throws Exception {
        Setup setup = setup();
        mockMvc.perform(patch(BASE + setup.fixture().orgId() + "/suppliers/" + setup.supplierId())
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-inactive-supplier\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("SUPPLIER_INACTIVE"));

        Setup other = setup();
        mockMvc.perform(patch(BASE + other.fixture().orgId() + "/branches/" + other.branchId())
                        .header("Authorization", bearer(other.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(ordersUrl(other))
                        .header("Authorization", bearer(other.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-inactive-branch\",\"supplierId\":\"" + other.supplierId() + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BRANCH_INACTIVE"));
    }

    @Test
    void addLineCapturesSnapshotsAndForbidsDuplicateItem() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-lines");

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + setup.itemId()
                                + "\",\"quantity\":\"25000.000000\",\"unitCost\":\"0.0180\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.purchaseOrder.version").value(1))
                .andExpect(jsonPath("$.lines[0].lineNumber").value(1))
                .andExpect(jsonPath("$.lines[0].itemCodeSnapshot").value("harina-trigo"))
                .andExpect(jsonPath("$.lines[0].itemNameSnapshot").value("Harina de trigo"))
                .andExpect(jsonPath("$.lines[0].unitSnapshot").value("GRAM"))
                .andExpect(jsonPath("$.lines[0].quantity").value("25000.000000"))
                .andExpect(jsonPath("$.lines[0].unitCost").value("0.0180"));

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.line.added");

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"inventoryItemId\":\"" + setup.itemId()
                                + "\",\"quantity\":\"1.000000\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_LINE_DUPLICATE_ITEM"));
    }

    @Test
    void snapshotSurvivesCatalogRename() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-snapshot");
        addLine(setup, orderId, setup.itemId().toString(), "10.000000", null, 0);

        mockMvc.perform(patch(BASE + setup.fixture().orgId() + "/inventory-items/" + setup.itemId())
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Harina de trigo premium\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(get(ordersUrl(setup) + "/" + orderId).header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.lines[0].itemNameSnapshot").value("Harina de trigo"));
    }

    @Test
    void lineMutationsBumpTheAggregateVersion() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-version");
        String lineId = addLine(setup, orderId, setup.itemId().toString(), "10.000000", null, 0);

        mockMvc.perform(patch(ordersUrl(setup) + "/" + orderId + "/lines/" + lineId)
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"quantity\":\"12.500000\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.version").value(2))
                .andExpect(jsonPath("$.lines[0].quantity").value("12.500000"))
                .andExpect(jsonPath("$.lines[0].unitCost").doesNotExist());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines/" + lineId + "/remove")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.version").value(3))
                .andExpect(jsonPath("$.lines").isEmpty());

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.line.removed");
    }

    @Test
    void updateLineCanKeepSetZeroAndClearUnitCost() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-unit-cost");
        String lineId = addLine(setup, orderId, setup.itemId().toString(), "10.000000", "0.0180", 0);

        // Omitir unitCost conserva el costo ya capturado (solo cambia la cantidad).
        mockMvc.perform(patch(ordersUrl(setup) + "/" + orderId + "/lines/" + lineId)
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"quantity\":\"11.000000\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.version").value(2))
                .andExpect(jsonPath("$.lines[0].quantity").value("11.000000"))
                .andExpect(jsonPath("$.lines[0].unitCost").value("0.0180"));

        // Un null explícito limpia el costo a NULL, también en la base.
        mockMvc.perform(patch(ordersUrl(setup) + "/" + orderId + "/lines/" + lineId)
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2,\"unitCost\":null}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.version").value(3))
                .andExpect(jsonPath("$.lines[0].unitCost").doesNotExist());
        assertThat(jdbcTemplate.queryForObject(
                        "select unit_cost from purchase_order_lines where id = ?",
                        BigDecimal.class,
                        UUID.fromString(lineId)))
                .isNull();

        // Un cero explícito es un valor, no una limpieza.
        mockMvc.perform(patch(ordersUrl(setup) + "/" + orderId + "/lines/" + lineId)
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":3,\"unitCost\":\"0.0000\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.version").value(4))
                .andExpect(jsonPath("$.lines[0].unitCost").value("0.0000"));

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.line.updated");
    }

    @Test
    void updateHeaderChangesSupplierAndRejectsStaleVersion() throws Exception {
        Setup setupA = setup();
        Setup setupB = setup();
        String orderId = createOrder(setupA, "po-header");

        mockMvc.perform(patch(ordersUrl(setupA) + "/" + orderId)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"supplierId\":\"" + setupB.supplierId() + "\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("SUPPLIER_NOT_FOUND"));

        mockMvc.perform(patch(ordersUrl(setupA) + "/" + orderId)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"supplierId\":\"" + setupA.supplierId() + "\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.version").value(0));

        mockMvc.perform(patch(ordersUrl(setupA) + "/" + orderId)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"CANCELLED\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("DRAFT"));

        mockMvc.perform(patch(ordersUrl(setupA) + "/" + orderId)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":7}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void listFiltersByStatusSupplierAndQueryWithStablePagination() throws Exception {
        Setup setup = setup();
        Setup other = setup();
        String first = createOrder(setup, "po-centro-2026");
        createOrder(setup, "po-norte-2026");
        createOrder(other, "po-otra-organizacion");

        mockMvc.perform(get(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .param("q", "CENTRO")
                        .param("supplierId", setup.supplierId().toString())
                        .param("status", "DRAFT")
                        .param("size", "1"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].id").value(first))
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.page").value(0))
                .andExpect(jsonPath("$.size").value(1));

        mockMvc.perform(get(ordersUrl(setup)).header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(2));
    }

    @Test
    void listRejectsInvalidPaginationParameters() throws Exception {
        Setup setup = setup();
        String url = ordersUrl(setup);
        mockMvc.perform(get(url).header("Authorization", bearer(setup.token())).param("page", "-1"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
        mockMvc.perform(get(url).header("Authorization", bearer(setup.token())).param("size", "0"))
                .andExpect(status().isBadRequest());
        mockMvc.perform(get(url).header("Authorization", bearer(setup.token())).param("size", "101"))
                .andExpect(status().isBadRequest());
    }

    @Test
    void orderIsNeverAccessibleFromAnotherOrganizationOrBranch() throws Exception {
        Setup setupA = setup();
        Setup setupB = setup();
        String orderId = createOrder(setupA, "po-aislada");
        String lineId = addLine(setupA, orderId, setupA.itemId().toString(), "5.000000", null, 0);

        mockMvc.perform(get(BASE + setupB.fixture().orgId() + "/branches/" + setupB.branchId()
                        + "/purchase-orders/" + orderId)
                        .header("Authorization", bearer(setupB.token())))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_FOUND"));

        Branch secondBranch = branchService.create(setupA.fixture().orgId(), "Norte", "sucursal-norte", null);
        String crossBranchUrl = BASE + setupA.fixture().orgId() + "/branches/" + secondBranch.getId()
                + "/purchase-orders/" + orderId;
        mockMvc.perform(get(crossBranchUrl).header("Authorization", bearer(setupA.token())))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_FOUND"));
        mockMvc.perform(patch(crossBranchUrl)
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isNotFound());

        mockMvc.perform(patch(BASE + setupB.fixture().orgId() + "/branches/" + setupB.branchId()
                        + "/purchase-orders/" + UUID.randomUUID() + "/lines/" + lineId)
                        .header("Authorization", bearer(setupB.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"quantity\":\"1.000000\"}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void addLineRejectsInactiveInventoryItem() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-inactive-item");
        mockMvc.perform(patch(BASE + setup.fixture().orgId() + "/inventory-items/" + setup.itemId())
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + setup.itemId()
                                + "\",\"quantity\":\"1.000000\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INVENTORY_ITEM_INACTIVE"));
    }

    @Test
    void rbacSeparatesReadFromWrite() throws Exception {
        RbacFixture reader = fixture();
        Setup readerSetup = setup(reader, null);
        createRoleAndAssign(reader, "order-reader", "Order Reader", "purchasing.order.read");
        String readerToken = login(reader);
        String readerUrl = ordersUrl(readerSetup);
        mockMvc.perform(get(readerUrl).header("Authorization", bearer(readerToken))).andExpect(status().isOk());
        mockMvc.perform(post(readerUrl)
                        .header("Authorization", bearer(readerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-denied\",\"supplierId\":\"" + readerSetup.supplierId() + "\"}"))
                .andExpect(status().isForbidden());

        RbacFixture writer = fixture();
        Setup writerSetup = setup(writer, null);
        createRoleAndAssign(writer, "order-writer", "Order Writer", "purchasing.order.write");
        String writerToken = login(writer);
        String writerUrl = ordersUrl(writerSetup);
        mockMvc.perform(post(writerUrl)
                        .header("Authorization", bearer(writerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-allowed\",\"supplierId\":\"" + writerSetup.supplierId() + "\"}"))
                .andExpect(status().isCreated());
        mockMvc.perform(get(writerUrl).header("Authorization", bearer(writerToken)))
                .andExpect(status().isForbidden());
    }

    @Test
    void readsDoNotWriteAuditEventsAndNoStockChanges() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-read-only");
        addLine(setup, orderId, setup.itemId().toString(), "3.000000", null, 0);
        long before = auditCount(setup.fixture().orgId());

        mockMvc.perform(get(ordersUrl(setup)).header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk());
        mockMvc.perform(get(ordersUrl(setup) + "/" + orderId).header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk());
        assertThat(auditCount(setup.fixture().orgId())).isEqualTo(before);

        Long movements = jdbcTemplate.queryForObject(
                "select count(*) from inventory_movements where organization_id = ?",
                Long.class,
                setup.fixture().orgId());
        Long balances = jdbcTemplate.queryForObject(
                "select count(*) from branch_inventory where organization_id = ?",
                Long.class,
                setup.fixture().orgId());
        assertThat(movements).isZero();
        assertThat(balances).isZero();
    }

    @Test
    void nonDraftOrdersCannotBeEdited() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-not-draft");
        jdbcTemplate.update(
                "update purchase_orders set status = 'ORDERED', issued_at = now(), issued_by_user_id = ? where id = ?",
                setup.fixture().userId(),
                UUID.fromString(orderId));

        mockMvc.perform(patch(ordersUrl(setup) + "/" + orderId)
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_DRAFT"));
    }
}
