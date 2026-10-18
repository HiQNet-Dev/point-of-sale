package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
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
 * Pruebas funcionales de la recepción de mercancía (parcial y completa) sobre la
 * cadena de seguridad real y PostgreSQL real: actualización atómica de
 * existencias, ledger append-only, estados de la orden, idempotencia, auditoría y
 * RBAC de lectura/escritura separados.
 */
class PurchaseReceiptApiTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private InventoryItemService inventoryItemService;

    private record Setup(RbacFixture fixture, String token, UUID branchId, UUID supplierId, UUID itemId) {
    }

    private record IssuedOrder(String orderId, UUID lineId) {
    }

    private Setup setup() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
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

    private String receiptsUrl(Setup setup, String orderId) {
        return ordersUrl(setup) + "/" + orderId + "/receipts";
    }

    private IssuedOrder issuedOrder(Setup setup, String code, String quantity) throws Exception {
        MvcResult created = mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        String orderId = objectMapper
                .readTree(created.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();

        MvcResult line = mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + setup.itemId() + "\",\"quantity\":\""
                                + quantity + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        UUID lineId = UUID.fromString(objectMapper
                .readTree(line.getResponse().getContentAsString())
                .get("lines")
                .get(0)
                .get("id")
                .asText());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("ORDERED"))
                .andExpect(jsonPath("$.purchaseOrder.version").value(2));
        return new IssuedOrder(orderId, lineId);
    }

    private MvcResult postReceipt(
            Setup setup, String orderId, String key, long expectedVersion, UUID lineId, String quantity)
            throws Exception {
        return mockMvc.perform(post(receiptsUrl(setup, orderId))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + expectedVersion + ",\"idempotencyKey\":\"" + key
                                + "\",\"lines\":[{\"purchaseOrderLineId\":\"" + lineId + "\",\"quantity\":\"" + quantity
                                + "\"}]}"))
                .andReturn();
    }

    private BigDecimal balanceOnHand(Setup setup) {
        return jdbcTemplate.queryForObject(
                "select quantity_on_hand from branch_inventory where branch_id = ? and inventory_item_id = ?",
                BigDecimal.class,
                setup.branchId(),
                setup.itemId());
    }

    private long countMovements(Setup setup, String type) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from inventory_movements where organization_id = ? and type = ?",
                Long.class,
                setup.fixture().orgId(),
                type);
        return count == null ? 0 : count;
    }

    private long countReceipts(Setup setup) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from purchase_receipts where organization_id = ?",
                Long.class,
                setup.fixture().orgId());
        return count == null ? 0 : count;
    }

    private long auditCountByAction(Setup setup, String action) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from audit_events where organization_id = ? and action = ?",
                Long.class,
                setup.fixture().orgId(),
                action);
        return count == null ? 0 : count;
    }

    @Test
    void fullReceiptPostsStockCompletesOrderAndAudits() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-full", "10.000000");
        long before = auditCount(setup.fixture().orgId());

        MvcResult result = postReceipt(setup, order.orderId(), "k-full", 2, order.lineId(), "10.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(201);
        assertThat(objectMapper.readTree(result.getResponse().getContentAsString())
                        .get("purchaseOrder")
                        .get("status")
                        .asText())
                .isEqualTo("RECEIVED");

        mockMvc.perform(get(receiptsUrl(setup, order.orderId()))
                        .header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(1))
                .andExpect(jsonPath("$[0].purchaseOrderId").value(order.orderId()))
                .andExpect(jsonPath("$[0].idempotencyKey").value("k-full"));

        mockMvc.perform(get(receiptsUrl(setup, order.orderId()) + "/" + objectMapper
                                .readTree(result.getResponse().getContentAsString())
                                .get("purchaseReceipt")
                                .get("id")
                                .asText())
                        .header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.lines.length()").value(1))
                .andExpect(jsonPath("$.lines[0].quantityReceived").value("10.000000"))
                .andExpect(jsonPath("$.purchaseOrder.status").value("RECEIVED"))
                .andExpect(jsonPath("$.purchaseOrder.version").value(3));

        mockMvc.perform(get(ordersUrl(setup) + "/" + order.orderId() + "/receipt-status")
                        .header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("RECEIVED"))
                .andExpect(jsonPath("$.lines[0].quantityReceived").value("10.000000"))
                .andExpect(jsonPath("$.lines[0].quantityPending").value("0.000000"));

        assertThat(balanceOnHand(setup)).isEqualByComparingTo("10.000000");
        assertThat(countMovements(setup, "PURCHASE_RECEIPT")).isEqualTo(1);
        assertThat(countMovements(setup, "ADJUSTMENT")).isZero();
        assertThat(auditCountByAction(setup, "purchase_receipt.posted")).isEqualTo(1);
        assertThat(auditCount(setup.fixture().orgId())).isEqualTo(before + 1);
        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_receipt.posted");
        assertThat(latestAudit(setup.fixture().orgId()).branchId()).isEqualTo(setup.branchId());
        assertThat(metadataOf(latestAudit(setup.fixture().orgId()))).containsEntry("lineCount", 1);
    }

    @Test
    void partialReceiptLeavesPendingAndSecondReceiptCompletes() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-partial", "10.000000");

        MvcResult partial = postReceipt(setup, order.orderId(), "k-1", 2, order.lineId(), "4.000000");
        assertThat(partial.getResponse().getStatus()).isEqualTo(201);
        assertThat(objectMapper.readTree(partial.getResponse().getContentAsString())
                        .get("purchaseOrder")
                        .get("status")
                        .asText())
                .isEqualTo("PARTIALLY_RECEIVED");
        assertThat(balanceOnHand(setup)).isEqualByComparingTo("4.000000");

        mockMvc.perform(get(ordersUrl(setup) + "/" + order.orderId() + "/receipt-status")
                        .header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("PARTIALLY_RECEIVED"))
                .andExpect(jsonPath("$.lines[0].quantityPending").value("6.000000"));

        MvcResult completed = postReceipt(setup, order.orderId(), "k-2", 3, order.lineId(), "6.000000");
        assertThat(completed.getResponse().getStatus()).isEqualTo(201);
        assertThat(objectMapper.readTree(completed.getResponse().getContentAsString())
                        .get("purchaseOrder")
                        .get("status")
                        .asText())
                .isEqualTo("RECEIVED");
        assertThat(balanceOnHand(setup)).isEqualByComparingTo("10.000000");
        assertThat(countMovements(setup, "PURCHASE_RECEIPT")).isEqualTo(2);
    }

    @Test
    void overReceiptIsRejectedAtomically() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-over", "10.000000");

        MvcResult result = postReceipt(setup, order.orderId(), "k-over", 2, order.lineId(), "11.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(409);
        assertThat(objectMapper.readTree(result.getResponse().getContentAsString()).get("code").asText())
                .isEqualTo("PURCHASE_RECEIPT_EXCEEDS_ORDERED");

        assertThat(countReceipts(setup)).isZero();
        assertThat(countMovements(setup, "PURCHASE_RECEIPT")).isZero();
        mockMvc.perform(get(ordersUrl(setup) + "/" + order.orderId())
                        .header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("ORDERED"))
                .andExpect(jsonPath("$.purchaseOrder.version").value(2));
    }

    @Test
    void idempotentReplayReturnsOriginalWithoutNewEffects() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-replay", "10.000000");

        MvcResult first = postReceipt(setup, order.orderId(), "k-replay", 2, order.lineId(), "4.000000");
        assertThat(first.getResponse().getStatus()).isEqualTo(201);
        String firstBody = first.getResponse().getContentAsString();

        MvcResult replay = postReceipt(setup, order.orderId(), "k-replay", 2, order.lineId(), "4.000000");
        assertThat(replay.getResponse().getStatus()).isEqualTo(200);
        var firstJson = objectMapper.readTree(firstBody);
        var replayJson = objectMapper.readTree(replay.getResponse().getContentAsString());
        assertThat(replayJson.get("purchaseReceipt").get("id").asText())
                .isEqualTo(firstJson.get("purchaseReceipt").get("id").asText());
        assertThat(replayJson.get("purchaseOrder").get("version").asLong())
                .isEqualTo(firstJson.get("purchaseOrder").get("version").asLong());
        assertThat(replayJson.get("purchaseOrder").get("status").asText())
                .isEqualTo(firstJson.get("purchaseOrder").get("status").asText());
        assertThat(replayJson.get("lines")).isEqualTo(firstJson.get("lines"));

        assertThat(countReceipts(setup)).isEqualTo(1);
        assertThat(countMovements(setup, "PURCHASE_RECEIPT")).isEqualTo(1);
        assertThat(auditCountByAction(setup, "purchase_receipt.posted")).isEqualTo(1);
        assertThat(balanceOnHand(setup)).isEqualByComparingTo("4.000000");
    }

    @Test
    void reusedKeyWithDifferentPayloadConflicts() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-conflict", "10.000000");
        postReceipt(setup, order.orderId(), "k-conflict", 2, order.lineId(), "4.000000");

        MvcResult conflict = postReceipt(setup, order.orderId(), "k-conflict", 3, order.lineId(), "6.000000");
        assertThat(conflict.getResponse().getStatus()).isEqualTo(409);
        assertThat(objectMapper.readTree(conflict.getResponse().getContentAsString())
                        .get("code")
                        .asText())
                .isEqualTo("PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT");

        assertThat(countReceipts(setup)).isEqualTo(1);
        assertThat(countMovements(setup, "PURCHASE_RECEIPT")).isEqualTo(1);
        assertThat(balanceOnHand(setup)).isEqualByComparingTo("4.000000");
    }

    @Test
    void draftOrderCannotBeReceived() throws Exception {
        Setup setup = setup();
        MvcResult created = mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-draft\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        String orderId = objectMapper
                .readTree(created.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();

        MvcResult result = postReceipt(setup, orderId, "k-draft", 0, UUID.randomUUID(), "1.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(409);
        assertThat(objectMapper.readTree(result.getResponse().getContentAsString()).get("code").asText())
                .isEqualTo("PURCHASE_ORDER_NOT_ISSUED");
        assertThat(countReceipts(setup)).isZero();
    }

    @Test
    void cancelledOrderCannotBeReceived() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-cancelled-receipt", "10.000000");
        mockMvc.perform(post(ordersUrl(setup) + "/" + order.orderId() + "/cancel")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("CANCELLED"));

        MvcResult result = postReceipt(setup, order.orderId(), "k-cancelled", 3, order.lineId(), "1.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(409);
        assertThat(objectMapper.readTree(result.getResponse().getContentAsString()).get("code").asText())
                .isEqualTo("PURCHASE_ORDER_ALREADY_CANCELLED");
    }

    @Test
    void cancelIsRejectedWhenReceiptsExist() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-has-receipts", "10.000000");
        postReceipt(setup, order.orderId(), "k-has-receipts", 2, order.lineId(), "4.000000");

        MvcResult result = mockMvc.perform(post(ordersUrl(setup) + "/" + order.orderId() + "/cancel")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":3}"))
                .andReturn();
        assertThat(result.getResponse().getStatus()).isEqualTo(409);
        assertThat(objectMapper.readTree(result.getResponse().getContentAsString()).get("code").asText())
                .isEqualTo("PURCHASE_ORDER_HAS_RECEIPTS");

        mockMvc.perform(get(ordersUrl(setup) + "/" + order.orderId())
                        .header("Authorization", bearer(setup.token())))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("PARTIALLY_RECEIVED"));
    }

    @Test
    void staleExpectedVersionIsRejected() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-stale", "10.000000");

        MvcResult result = postReceipt(setup, order.orderId(), "k-stale", 1, order.lineId(), "4.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(409);
        assertThat(objectMapper.readTree(result.getResponse().getContentAsString()).get("code").asText())
                .isEqualTo("CONCURRENT_MODIFICATION");
        assertThat(countReceipts(setup)).isZero();
    }

    @Test
    void unknownLineIsRejected() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-unknown-line", "10.000000");

        MvcResult result = postReceipt(setup, order.orderId(), "k-unknown-line", 2, UUID.randomUUID(), "1.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(404);
        assertThat(objectMapper.readTree(result.getResponse().getContentAsString()).get("code").asText())
                .isEqualTo("PURCHASE_ORDER_LINE_NOT_FOUND");
    }

    @Test
    void receiptWriteRequiresPermissionAndReadIsSeparate() throws Exception {
        RbacFixture fixture = fixture();
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier =
                supplierService.create(fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        InventoryItem item =
                inventoryItemService.create(fixture.orgId(), "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
        createRoleAndAssign(
                fixture,
                "receipt-reader",
                "Receipt Reader",
                "purchasing.order.read",
                "purchasing.order.write",
                "purchasing.order.issue",
                "purchasing.receipt.read");
        String token = login(fixture);
        Setup setup = new Setup(fixture, token, branch.getId(), supplier.getId(), item.getId());
        IssuedOrder order = issuedOrder(setup, "po-rbac", "10.000000");

        mockMvc.perform(get(ordersUrl(setup) + "/" + order.orderId() + "/receipt-status")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk());

        MvcResult forbidden = postReceipt(setup, order.orderId(), "k-rbac", 2, order.lineId(), "1.000000");
        assertThat(forbidden.getResponse().getStatus()).isEqualTo(403);
        assertThat(countReceipts(setup)).isZero();
    }

    @Test
    void receiptLookupIsScopedToItsOrder() throws Exception {
        Setup setup = setup();
        IssuedOrder first = issuedOrder(setup, "po-scope-1", "10.000000");
        IssuedOrder second = issuedOrder(setup, "po-scope-2", "10.000000");
        MvcResult receipt = postReceipt(setup, first.orderId(), "k-scope", 2, first.lineId(), "4.000000");
        String receiptId = objectMapper
                .readTree(receipt.getResponse().getContentAsString())
                .get("purchaseReceipt")
                .get("id")
                .asText();

        mockMvc.perform(get(receiptsUrl(setup, second.orderId()) + "/" + receiptId)
                        .header("Authorization", bearer(setup.token())))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("PURCHASE_RECEIPT_NOT_FOUND"));
    }
}
