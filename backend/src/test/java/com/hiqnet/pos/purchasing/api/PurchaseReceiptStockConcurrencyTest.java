package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Concurrencia real de la recepción contra otras mutaciones de existencia.
 *
 * <p>Coordina recepción con venta, con ajuste manual, con emisión/cancelación de
 * la orden y con la desactivación del insumo; y dos recepciones simultáneas sobre
 * órdenes distintas que comparten insumos con líneas en orden inverso para probar
 * que el orden de bloqueo determinista por articulo evita interbloqueos y pérdidas
 * de actualización. Cada escenario verifica el resultado final en PostgreSQL.</p>
 */
class PurchaseReceiptStockConcurrencyTest extends AbstractPurchaseReceiptTest {

    @Test
    void receiptAndSaleSequencesProduceCoherentBalance() throws Exception {
        Scope scope = scope();
        String registerId = register(scope, "seq-reg");
        String shiftId = shift(scope, registerId);
        String productId = product(scope, "SEQ-LATTE");
        recipe(scope, productId, scope.itemA());
        seedBalance(scope, scope.itemA(), "0");
        Order first = issuedOrder(scope, "po-seq-first", "10.000000");
        Order second = issuedOrder(scope, "po-seq-second", "10.000000");

        // Recepción antes de la venta.
        assertThat(postReceipt(scope, first.id(), "k-seq-first", first.version(), first.lineIds().get(0), "5.000000")
                        .getResponse()
                        .getStatus())
                .isEqualTo(201);
        String saleA = sale(scope, registerId, shiftId);
        addProduct(scope, registerId, shiftId, saleA, productId);
        assertThat(checkout(scope, registerId, shiftId, saleA)).isEqualTo(200);
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("4.000000");

        // Venta antes de la recepción.
        String saleB = sale(scope, registerId, shiftId);
        addProduct(scope, registerId, shiftId, saleB, productId);
        assertThat(checkout(scope, registerId, shiftId, saleB)).isEqualTo(200);
        assertThat(postReceipt(scope, second.id(), "k-seq-second", second.version(), second.lineIds().get(0), "5.000000")
                        .getResponse()
                        .getStatus())
                .isEqualTo(201);

        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("8.000000");
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(2);
        assertThat(countMovements(scope, "SALE_CONSUMPTION")).isEqualTo(2);
    }

    @Test
    void concurrentReceiptAndSaleKeepLedgerCoherent() throws Exception {
        Scope scope = scope();
        String registerId = register(scope, "race-reg");
        String shiftId = shift(scope, registerId);
        String productId = product(scope, "RACE-LATTE");
        recipe(scope, productId, scope.itemA());
        seedBalance(scope, scope.itemA(), "4");
        Order order = issuedOrder(scope, "po-race-sale", "10.000000");
        String saleId = sale(scope, registerId, shiftId);
        addProduct(scope, registerId, shiftId, saleId, productId);

        List<Integer> results = runParallel(List.of(
                () -> postReceipt(scope, order.id(), "k-race-sale", order.version(), order.lineIds().get(0), "5.000000")
                        .getResponse()
                        .getStatus(),
                () -> checkout(scope, registerId, shiftId, saleId)));

        assertThat(results).containsExactlyInAnyOrder(201, 200);
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("8.000000");
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
        assertThat(countMovements(scope, "SALE_CONSUMPTION")).isEqualTo(1);
    }

    @Test
    void receiptAndAdjustmentSerializeWithoutLostUpdate() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-race-adjust", "10.000000");
        seedBalance(scope, scope.itemA(), "3");
        long balanceVersion = jdbcTemplate.queryForObject(
                "select version from branch_inventory where branch_id = ? and inventory_item_id = ?",
                Long.class,
                scope.branchId(),
                scope.itemA());

        List<Integer> results = runParallel(List.of(
                () -> postReceipt(scope, order.id(), "k-race-adjust", order.version(), order.lineIds().get(0), "5.000000")
                        .getResponse()
                        .getStatus(),
                () -> mockMvc.perform(post(BASE + scope.fixture().orgId() + "/branches/" + scope.branchId()
                                        + "/inventory-items/" + scope.itemA() + "/adjustments")
                                .header("Authorization", bearer(scope.token()))
                                .contentType(MediaType.APPLICATION_JSON)
                                .content("{\"expectedVersion\":" + balanceVersion
                                        + ",\"quantityDelta\":\"-2.000000\",\"reason\":\"merma\"}"))
                        .andReturn()
                        .getResponse()
                        .getStatus()));

        int receiptStatus = results.get(0);
        int adjustmentStatus = results.get(1);
        assertThat(receiptStatus).isEqualTo(201);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
        if (adjustmentStatus == 200) {
            assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("6.000000");
            assertThat(countMovements(scope, "ADJUSTMENT")).isEqualTo(1);
        } else {
            // El ajuste leyó una versión obsoleta: la recepción ya había movido el balance.
            assertThat(adjustmentStatus).isEqualTo(409);
            assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("8.000000");
            assertThat(countMovements(scope, "ADJUSTMENT")).isZero();
        }
    }

    @Test
    void receiptAndCancelRaceNeverLeavesCancelledOrderWithStock() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-race-cancel", "10.000000");

        List<Integer> results = runParallel(List.of(
                () -> postReceipt(scope, order.id(), "k-race-cancel", order.version(), order.lineIds().get(0), "4.000000")
                        .getResponse()
                        .getStatus(),
                () -> mockMvc.perform(post(ordersUrl(scope) + "/" + order.id() + "/cancel")
                                .header("Authorization", bearer(scope.token()))
                                .contentType(MediaType.APPLICATION_JSON)
                                .content("{\"expectedVersion\":2}"))
                        .andReturn()
                        .getResponse()
                        .getStatus()));

        if ("CANCELLED".equals(orderStatus(order.id()))) {
            assertThat(results).containsExactlyInAnyOrder(200, 409);
            assertThat(countReceipts(scope)).isZero();
            assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isZero();
            assertThat(balance(scope, scope.itemA())).isNull();
        } else {
            assertThat(results).containsExactlyInAnyOrder(201, 409);
            assertThat(orderStatus(order.id())).isEqualTo("PARTIALLY_RECEIVED");
            assertThat(countReceipts(scope)).isEqualTo(1);
            assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
            assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("4.000000");
        }
    }

    @Test
    void concurrentReceiptsOnDifferentOrdersLockItemsDeterministically() throws Exception {
        Scope scope = scope();
        Order first = issuedOrder(scope, "po-lock-first", "10.000000", "10.000000");
        Order second = issuedOrder(scope, "po-lock-second", "10.000000", "10.000000");

        // Misma sucursal e insumos, pero las líneas llegan en orden inverso: si el
        // orden de bloqueo por artículo no fuese determinista, podría interbloquear.
        List<Integer> results = runParallel(List.of(
                () -> postReceipt(
                                scope,
                                first.id(),
                                "k-lock-first",
                                first.version(),
                                first.lineIds(),
                                List.of("2.000000", "2.000000"))
                        .getResponse()
                        .getStatus(),
                () -> postReceipt(
                                scope,
                                second.id(),
                                "k-lock-second",
                                second.version(),
                                List.of(second.lineIds().get(1), second.lineIds().get(0)),
                                List.of("3.000000", "3.000000"))
                        .getResponse()
                        .getStatus()));

        assertThat(results).containsExactly(201, 201);
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("5.000000");
        assertThat(balance(scope, scope.itemB())).isEqualByComparingTo("5.000000");
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(4);
        assertThat(countReceipts(scope)).isEqualTo(2);
    }

    @Test
    void receiptCoheresWithConcurrentItemDeactivation() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-race-item", "10.000000");
        seedBalance(scope, scope.itemA(), "0");
        long itemVersion = itemVersion(scope.itemA());

        List<Integer> results = runParallel(List.of(
                () -> postReceipt(scope, order.id(), "k-race-item", order.version(), order.lineIds().get(0), "4.000000")
                        .getResponse()
                        .getStatus(),
                () -> mockMvc.perform(patch(BASE + scope.fixture().orgId() + "/inventory-items/" + scope.itemA())
                                .header("Authorization", bearer(scope.token()))
                                .contentType(MediaType.APPLICATION_JSON)
                                .content("{\"expectedVersion\":" + itemVersion + ",\"status\":\"INACTIVE\"}"))
                        .andReturn()
                        .getResponse()
                        .getStatus()));

        int receiptStatus = results.get(0);
        if (receiptStatus == 201) {
            // La recepción validó el insumo ACTIVO bajo bloqueo: hay stock real.
            assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("4.000000");
            assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
        } else {
            assertThat(receiptStatus).isEqualTo(409);
            assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isZero();
            assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("0.000000");
        }
    }

    // --- venta: registro, turno, producto, receta y salida -----------------------

    private String register(Scope scope, String code) throws Exception {
        return id(mockMvc.perform(post(BASE + scope.fixture().orgId() + "/branches/" + scope.branchId()
                                + "/cash-registers")
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"Register\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString());
    }

    private String shift(Scope scope, String registerId) throws Exception {
        return id(mockMvc.perform(post(BASE + scope.fixture().orgId() + "/branches/" + scope.branchId()
                                + "/cash-registers/" + registerId + "/cash-shifts")
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":0}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString());
    }

    private String product(Scope scope, String sku) throws Exception {
        return id(mockMvc.perform(post(BASE + scope.fixture().orgId() + "/products")
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"" + sku + "\",\"name\":\"" + sku + "\",\"unitPrice\":0.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString());
    }

    private void recipe(Scope scope, String productId, UUID itemId) throws Exception {
        mockMvc.perform(put(BASE + scope.fixture().orgId() + "/products/" + productId + "/inventory-recipe")
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"ACTIVE\",\"components\":[{\"inventoryItemId\":\""
                                + itemId + "\",\"quantity\":\"1\"}]}"))
                .andExpect(status().isOk());
    }

    private String sale(Scope scope, String registerId, String shiftId) throws Exception {
        return id(mockMvc.perform(post(salesUrl(scope, registerId, shiftId))
                        .header("Authorization", bearer(scope.token())))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString());
    }

    private void addProduct(Scope scope, String registerId, String shiftId, String saleId, String productId)
            throws Exception {
        mockMvc.perform(post(salesUrl(scope, registerId, shiftId) + "/" + saleId + "/items")
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId
                                + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
    }

    private int checkout(Scope scope, String registerId, String shiftId, String saleId) throws Exception {
        long version = jdbcTemplate.queryForObject(
                "select version from sales where id = ?", Long.class, UUID.fromString(saleId));
        return mockMvc.perform(post(salesUrl(scope, registerId, shiftId) + "/" + saleId + "/checkout")
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":" + version + ",\"payments\":[]}"))
                .andReturn()
                .getResponse()
                .getStatus();
    }

    private static String salesUrl(Scope scope, String registerId, String shiftId) {
        return BASE + scope.fixture().orgId() + "/branches/" + scope.branchId() + "/cash-registers/" + registerId
                + "/cash-shifts/" + shiftId + "/sales";
    }

    private String id(String json) throws Exception {
        return objectMapper.readTree(json).get("id").asText();
    }

    private List<Integer> runParallel(List<Callable<Integer>> actions) throws Exception {
        CountDownLatch ready = new CountDownLatch(actions.size());
        CountDownLatch start = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(actions.size());
        try {
            List<Future<Integer>> futures = new ArrayList<>();
            for (Callable<Integer> action : actions) {
                futures.add(pool.submit(() -> {
                    ready.countDown();
                    start.await();
                    return action.call();
                }));
            }
            assertThat(ready.await(10, TimeUnit.SECONDS)).isTrue();
            start.countDown();
            List<Integer> results = new ArrayList<>();
            for (Future<Integer> future : futures) {
                results.add(future.get(30, TimeUnit.SECONDS));
            }
            return results;
        } finally {
            start.countDown();
            pool.shutdownNow();
        }
    }
}
