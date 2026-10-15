package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import java.util.ArrayList;
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
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Concurrencia real sobre PostgreSQL: el punto de sincronización del agregado es
 * la fila de cabecera bloqueada de forma pesimista más {@code @Version}.
 */
class PurchaseOrderConcurrencyTest extends AbstractRbacIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";
    private static final int THREADS = 4;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private InventoryItemService inventoryItemService;

    private record Fixture(RbacFixture rbac, String token, UUID branchId, UUID supplierId, List<UUID> itemIds) {
    }

    private Fixture prepare(int itemCount) throws Exception {
        RbacFixture rbac = fixture();
        String token = adminToken(rbac);
        Branch branch = branchService.create(rbac.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                rbac.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        List<UUID> items = new ArrayList<>();
        for (int index = 0; index < itemCount; index++) {
            items.add(inventoryItemService
                    .create(rbac.orgId(), "item-" + index, "Item " + index, null, InventoryBaseUnit.UNIT, false)
                    .getId());
        }
        return new Fixture(rbac, token, branch.getId(), supplier.getId(), items);
    }

    private String ordersUrl(Fixture fixture) {
        return BASE + fixture.rbac().orgId() + "/branches/" + fixture.branchId() + "/purchase-orders";
    }

    private String createOrder(Fixture fixture, String code) throws Exception {
        MvcResult result = mockMvc.perform(post(ordersUrl(fixture))
                        .header("Authorization", bearer(fixture.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + fixture.supplierId() + "\"}"))
                .andExpect(org.springframework.test.web.servlet.result.MockMvcResultMatchers.status().isCreated())
                .andReturn();
        return objectMapper.readTree(result.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();
    }

    @Test
    void parallelCreatesOfTheSameCodeYieldOneCreatedAndControlledConflicts() throws Exception {
        Fixture fixture = prepare(1);
        String url = ordersUrl(fixture);
        String body = "{\"code\":\"po-race\",\"supplierId\":\"" + fixture.supplierId() + "\"}";

        List<Integer> statuses = runParallel(THREADS, () -> mockMvc.perform(post(url)
                        .header("Authorization", bearer(fixture.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andReturn()
                .getResponse()
                .getStatus());

        assertThat(statuses.stream().filter(status -> status == 201).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(status -> status == 409).count()).isEqualTo(THREADS - 1);
        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from purchase_orders where organization_id = ? and code = 'po-race'",
                Long.class,
                fixture.rbac().orgId());
        assertThat(stored).isEqualTo(1);
    }

    @Test
    void parallelLineAddsSerializeOnTheAggregateVersionWithoutLostUpdates() throws Exception {
        Fixture fixture = prepare(THREADS);
        String orderId = createOrder(fixture, "po-lines-race");
        String url = ordersUrl(fixture) + "/" + orderId + "/lines";
        java.util.concurrent.atomic.AtomicInteger nextItem = new java.util.concurrent.atomic.AtomicInteger(0);

        List<Integer> statuses = runParallel(THREADS, () -> {
            UUID itemId = fixture.itemIds().get(nextItem.getAndIncrement());
            return mockMvc.perform(post(url)
                            .header("Authorization", bearer(fixture.token()))
                            .contentType(MediaType.APPLICATION_JSON)
                            .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + itemId
                                    + "\",\"quantity\":\"1.000000\"}"))
                    .andReturn()
                    .getResponse()
                    .getStatus();
        });

        // Solo una alta puede ganar la versión 0: las mutaciones de partida incrementan
        // la versión del agregado, de modo que las otras tres quedan obsoletas.
        assertThat(statuses.stream().filter(status -> status == 201).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(status -> status == 409).count()).isEqualTo(THREADS - 1);
        Long lines = jdbcTemplate.queryForObject(
                "select count(*) from purchase_order_lines where purchase_order_id = ?",
                Long.class,
                UUID.fromString(orderId));
        assertThat(lines).isEqualTo(1);
        Long version = jdbcTemplate.queryForObject(
                "select version from purchase_orders where id = ?", Long.class, UUID.fromString(orderId));
        assertThat(version).isEqualTo(1);
    }

    @Test
    void parallelHeaderUpdatesDoNotLoseUpdates() throws Exception {
        Fixture fixture = prepare(1);
        Supplier otherSupplier = supplierService.create(
                fixture.rbac().orgId(), "proveedor-norte", "Proveedor Norte", null, null, null, null, null, null);
        String orderId = createOrder(fixture, "po-header-race");
        String url = ordersUrl(fixture) + "/" + orderId;
        String body = "{\"expectedVersion\":0,\"supplierId\":\"" + otherSupplier.getId() + "\"}";

        List<Integer> statuses = runParallel(THREADS, () -> mockMvc.perform(patch(url)
                        .header("Authorization", bearer(fixture.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andReturn()
                .getResponse()
                .getStatus());

        assertThat(statuses.stream().filter(status -> status == 200).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(status -> status == 409).count()).isEqualTo(THREADS - 1);
        Long version = jdbcTemplate.queryForObject(
                "select version from purchase_orders where id = ?", Long.class, UUID.fromString(orderId));
        assertThat(version).isEqualTo(1);
        String supplierSnapshot = jdbcTemplate.queryForObject(
                "select supplier_code_snapshot from purchase_orders where id = ?", String.class, UUID.fromString(orderId));
        assertThat(supplierSnapshot).isEqualTo("proveedor-norte");
    }

    private List<Integer> runParallel(int threads, Callable<Integer> action) throws Exception {
        CountDownLatch ready = new CountDownLatch(threads);
        CountDownLatch start = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(threads);
        List<Future<Integer>> results = new ArrayList<>();
        try {
            for (int index = 0; index < threads; index++) {
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
}
