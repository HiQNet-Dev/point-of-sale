package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Pruebas de concurrencia de la recepción de mercancía sobre PostgreSQL real.
 *
 * <p>Las barreras son deterministas ({@link CountDownLatch}), no {@code sleep}:
 * dos recepciones compiten por el mismo bloqueo de cabecera. La primera clave
 * idéntica se confirma una sola vez y el reintento devuelve el resultado
 * original; dos claves distintas con la misma versión esperada se serializan y
 * solo una muta el stock.</p>
 */
class PurchaseReceiptConcurrencyTest extends AbstractAuditIntegrationTest {

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

    private IssuedOrder issuedOrder(Setup setup, String code, String quantity) throws Exception {
        MvcResult created = mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
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
                .andReturn();
        return new IssuedOrder(orderId, lineId);
    }

    private int postReceipt(Setup setup, String orderId, String key, long expectedVersion, UUID lineId, String quantity)
            throws Exception {
        return mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/receipts")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + expectedVersion + ",\"idempotencyKey\":\"" + key
                                + "\",\"lines\":[{\"purchaseOrderLineId\":\"" + lineId + "\",\"quantity\":\"" + quantity
                                + "\"}]}"))
                .andReturn()
                .getResponse()
                .getStatus();
    }

    private List<Integer> runParallel(Callable<Integer> action) throws Exception {
        CountDownLatch ready = new CountDownLatch(2);
        CountDownLatch start = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(2);
        List<Future<Integer>> results = new ArrayList<>();
        try {
            for (int index = 0; index < 2; index++) {
                results.add(pool.submit(() -> {
                    ready.countDown();
                    start.await();
                    return action.call();
                }));
            }
            ready.await();
            start.countDown();
            List<Integer> statuses = new ArrayList<>();
            for (Future<Integer> result : results) {
                statuses.add(result.get());
            }
            return statuses;
        } finally {
            pool.shutdownNow();
        }
    }

    private long countReceipts(Setup setup) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from purchase_receipts where organization_id = ?",
                Long.class,
                setup.fixture().orgId());
        return count == null ? 0 : count;
    }

    private BigDecimal balanceOnHand(Setup setup) {
        return jdbcTemplate.queryForObject(
                "select quantity_on_hand from branch_inventory where branch_id = ? and inventory_item_id = ?",
                BigDecimal.class,
                setup.branchId(),
                setup.itemId());
    }

    @Test
    void simultaneousIdenticalReceiptsCommitExactlyOnce() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-race-same", "10.000000");
        String key = "k-race-same";

        List<Integer> statuses =
                runParallel(() -> postReceipt(setup, order.orderId(), key, 2, order.lineId(), "4.000000"));
        statuses.sort(Comparator.naturalOrder());

        assertThat(statuses).containsExactly(200, 201);
        assertThat(countReceipts(setup)).isEqualTo(1);
        Long movements = jdbcTemplate.queryForObject(
                "select count(*) from inventory_movements where organization_id = ? and type = 'PURCHASE_RECEIPT'",
                Long.class,
                setup.fixture().orgId());
        assertThat(movements).isEqualTo(1);
        Long audits = jdbcTemplate.queryForObject(
                "select count(*) from audit_events where organization_id = ? and action = 'purchase_receipt.posted'",
                Long.class,
                setup.fixture().orgId());
        assertThat(audits).isEqualTo(1);
        assertThat(balanceOnHand(setup)).isEqualByComparingTo("4.000000");
    }

    @Test
    void simultaneousDistinctReceiptsSerializeOnOrderLock() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-race-distinct", "10.000000");

        List<Integer> statuses = runParallel(() ->
                postReceipt(setup, order.orderId(), "k-race-" + UUID.randomUUID(), 2, order.lineId(), "5.000000"));
        statuses.sort(Comparator.naturalOrder());

        assertThat(statuses).containsExactly(201, 409);
        assertThat(countReceipts(setup)).isEqualTo(1);
        assertThat(balanceOnHand(setup)).isEqualByComparingTo("5.000000");
    }
}
