package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import java.math.BigDecimal;
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
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * Atomic checkout Inventory consumption: authoritative snapshots, STRICT versus
 * ALLOW_NEGATIVE policy, movement linkage, and stock-level contention across
 * concurrent checkouts on different registers.
 */
class SaleCheckoutInventoryConsumptionApiTest extends AbstractRbacIntegrationTest {

    private static final String ORG = "/api/v1/organizations/";

    @Autowired private BranchRepository branches;
    @Autowired private JdbcTemplate jdbc;

    @Test
    void strictCheckoutConsumesEachItemOnceAndRejectsSubsequentShortage() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, token, "consume-strict");
        String productId = product(fx, token, "LATTE", "Latte", "0.0000");
        String coffeeId = item(fx, token, "coffee", "Coffee", "GRAM", false);
        String cupId = item(fx, token, "cup", "Cup", "UNIT", false);
        replaceRecipe(fx, token, productId, 0, "ACTIVE",
                List.of(component(coffeeId, "18"), component(cupId, "1")));
        seedBalance(fx, scope.branchId(), coffeeId, "50");
        seedBalance(fx, scope.branchId(), cupId, "3");

        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, productId, 2, List.of()).andExpect(status().isOk());
        long version = saleVersion(saleId);
        checkout(scope, token, saleId, version, "[]").andExpect(status().isOk())
                .andExpect(jsonPath("$.sale.status").value("COMPLETED"));

        assertThat(quantity(scope.branchId(), coffeeId)).isEqualByComparingTo("14");
        assertThat(quantity(scope.branchId(), cupId)).isEqualByComparingTo("1");
        assertThat(consumptionMovements(saleId)).hasSize(2);
        assertThat(movementType(fx, scope.branchId(), coffeeId)).isEqualTo("SALE_CONSUMPTION");
        assertThat(movementDelta(fx, scope.branchId(), coffeeId)).isEqualByComparingTo("-36");
        assertThat(movementDelta(fx, scope.branchId(), cupId)).isEqualByComparingTo("-2");

        // Selling more than remains must fail before any write.
        String shortSale = sale(scope, token);
        addItem(scope, token, shortSale, 0, productId, 2, List.of()).andExpect(status().isOk());
        long shortVersion = saleVersion(shortSale);
        long movementsBefore = countMovements();
        checkout(scope, token, shortSale, shortVersion, "[]")
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INSUFFICIENT_INVENTORY"))
                .andExpect(jsonPath("$.details").isNotEmpty());
        assertThat(jdbc.queryForObject("select status from sales where id = ?", String.class,
                UUID.fromString(shortSale))).isEqualTo("OPEN");
        assertThat(countMovements()).isEqualTo(movementsBefore);
        assertThat(quantity(scope.branchId(), coffeeId)).isEqualByComparingTo("14");
        assertThat(quantity(scope.branchId(), cupId)).isEqualByComparingTo("1");
    }

    @Test
    void allowNegativePolicyPermitsNegativeBalanceAndRecordsMovement() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, token, "consume-allow");
        jdbc.update("update organizations set inventory_negative_stock_policy = 'ALLOW_NEGATIVE' where id = ?", fx.orgId());
        String productId = product(fx, token, "BREAD", "Bread", "0.0000");
        String flourId = item(fx, token, "flour", "Flour", "GRAM", false);
        replaceRecipe(fx, token, productId, 0, "ACTIVE", List.of(component(flourId, "100")));
        seedBalance(fx, scope.branchId(), flourId, "30");

        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, productId, 1, List.of()).andExpect(status().isOk());
        checkout(scope, token, saleId, saleVersion(saleId), "[]").andExpect(status().isOk())
                .andExpect(jsonPath("$.sale.status").value("COMPLETED"));

        assertThat(quantity(scope.branchId(), flourId)).isEqualByComparingTo("-70");
        assertThat(movementDelta(fx, scope.branchId(), flourId)).isEqualByComparingTo("-100");
        assertThat(consumptionMovements(saleId)).hasSize(1);
    }

    @Test
    void untrackedLinesConsumeNothingAndCreateNoBalanceRow() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, token, "consume-untracked");
        String productId = product(fx, token, "PLAIN", "Plain", "0.0000");

        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, productId, 3, List.of()).andExpect(status().isOk());
        checkout(scope, token, saleId, saleVersion(saleId), "[]").andExpect(status().isOk())
                .andExpect(jsonPath("$.sale.status").value("COMPLETED"));

        assertThat(jdbc.queryForObject("select count(*) from branch_inventory where branch_id = ?", Long.class,
                scope.branchId())).isZero();
        assertThat(consumptionMovements(saleId)).isEmpty();
    }

    @Test
    void aggregatedSnapshotsWithDisagreeingBaseUnitAreRejected() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Scope scope = scope(fx, token, "consume-mismatch");
        String productId = product(fx, token, "MISMATCH", "Mismatch", "0.0000");
        String itemId = item(fx, token, "mismatch-item", "Mismatch item", "GRAM", false);
        replaceRecipe(fx, token, productId, 0, "ACTIVE", List.of(component(itemId, "10")));
        seedBalance(fx, scope.branchId(), itemId, "100");

        String saleId = sale(scope, token);
        addItem(scope, token, saleId, 0, productId, 1, List.of()).andExpect(status().isOk());
        addItem(scope, token, saleId, 1, productId, 1, List.of()).andExpect(status().isOk());
        UUID oneComponent = jdbc.queryForObject(
                "select id from sale_item_inventory_components where inventory_item_id = ? order by sale_item_id limit 1",
                UUID.class, UUID.fromString(itemId));
        jdbc.update("update sale_item_inventory_components set base_unit_snapshot = 'MILLILITER' where id = ?", oneComponent);

        long movementsBefore = countMovements();
        checkout(scope, token, saleId, saleVersion(saleId), "[]")
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INVENTORY_SNAPSHOT_INCONSISTENT"));
        assertThat(jdbc.queryForObject("select status from sales where id = ?", String.class,
                UUID.fromString(saleId))).isEqualTo("OPEN");
        assertThat(countMovements()).isEqualTo(movementsBefore);
        assertThat(quantity(scope.branchId(), itemId)).isEqualByComparingTo("100");
    }

    @Test
    void concurrentCheckoutsAcrossRegistersConsumeStockExactlyOnce() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "consume-race");
        String registerA = register(fx, token, branchId, "race-a");
        String registerB = register(fx, token, branchId, "race-b");
        String shiftA = shift(fx, token, branchId, registerA);
        String shiftB = shift(fx, token, branchId, registerB);
        String productId = product(fx, token, "RACE", "Race", "0.0000");
        String itemId = item(fx, token, "race-item", "Race item", "GRAM", false);
        replaceRecipe(fx, token, productId, 0, "ACTIVE", List.of(component(itemId, "1")));
        seedBalance(fx, branchId, itemId, "1");

        String saleA = sale(new Scope(fx, branchId, registerA, shiftA), token);
        String saleB = sale(new Scope(fx, branchId, registerB, shiftB), token);
        addItem(new Scope(fx, branchId, registerA, shiftA), token, saleA, 0, productId, 1, List.of()).andExpect(status().isOk());
        addItem(new Scope(fx, branchId, registerB, shiftB), token, saleB, 0, productId, 1, List.of()).andExpect(status().isOk());

        List<Integer> statuses = runParallel(List.of(
                () -> checkoutStatus(new Scope(fx, branchId, registerA, shiftA), token, saleA, saleVersion(saleA)),
                () -> checkoutStatus(new Scope(fx, branchId, registerB, shiftB), token, saleB, saleVersion(saleB))));

        assertThat(statuses.stream().filter(status -> status == 200).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(status -> status == 409).count()).isEqualTo(1);
        assertThat(quantity(branchId, itemId)).isEqualByComparingTo("0");
        assertThat(jdbc.queryForObject(
                "select count(*) from inventory_movements where branch_id = ? and inventory_item_id = ? and type = 'SALE_CONSUMPTION'",
                Long.class, branchId, UUID.fromString(itemId))).isEqualTo(1L);
    }

    // --- helpers -----------------------------------------------------------------

    private static String component(String itemId, String quantity) {
        return "{\"inventoryItemId\":\"" + itemId + "\",\"quantity\":\"" + quantity + "\"}";
    }

    private Scope scope(RbacFixture fx, String token, String code) throws Exception {
        UUID branchId = createBranch(fx, code);
        String registerId = register(fx, token, branchId, code);
        String shiftId = shift(fx, token, branchId, registerId);
        return new Scope(fx, branchId, registerId, shiftId);
    }

    private UUID createBranch(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branches.saveAndFlush(Branch.create(organization, "Branch " + code, code, null)).getId();
    }

    private String register(RbacFixture fx, String token, UUID branchId, String code) throws Exception {
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/branches/" + branchId + "/cash-registers")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"Register\"}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private String shift(RbacFixture fx, String token, UUID branchId, String registerId) throws Exception {
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/branches/" + branchId + "/cash-registers/" + registerId + "/cash-shifts")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":0}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private String sale(Scope scope, String token) throws Exception {
        return id(mockMvc.perform(post(salesUrl(scope)).header("Authorization", bearer(token)))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private String product(RbacFixture fx, String token, String sku, String name, String price) throws Exception {
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/products").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"" + sku + "\",\"name\":\"" + name + "\",\"unitPrice\":" + price + "}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private String item(RbacFixture fx, String token, String code, String name, String unit, boolean fractional)
            throws Exception {
        return id(mockMvc.perform(post(ORG + fx.orgId() + "/inventory-items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + name + "\",\"baseUnit\":\"" + unit
                                + "\",\"fractionalUnitAllowed\":" + fractional + "}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString());
    }

    private void replaceRecipe(RbacFixture fx, String token, String productId, int version, String state,
            List<String> components) throws Exception {
        String body = "{\"expectedVersion\":" + version + ",\"status\":\"" + state
                + "\",\"components\":[" + String.join(",", components) + "]}";
        mockMvc.perform(put(ORG + fx.orgId() + "/products/" + productId + "/inventory-recipe")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content(body))
                .andExpect(status().isOk());
    }

    private org.springframework.test.web.servlet.ResultActions addItem(Scope scope, String token, String saleId,
            long saleVersion, String productId, int quantity, List<String> options) throws Exception {
        String ids = options.stream().map(id -> "\"" + id + "\"").collect(java.util.stream.Collectors.joining(","));
        return mockMvc.perform(post(salesUrl(scope) + "/" + saleId + "/items")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"expectedSaleVersion\":" + saleVersion + ",\"productId\":\"" + productId
                        + "\",\"quantity\":" + quantity + ",\"modifierOptionIds\":[" + ids + "]}"));
    }

    private org.springframework.test.web.servlet.ResultActions checkout(Scope scope, String token, String saleId,
            long version, String payments) throws Exception {
        return mockMvc.perform(post(salesUrl(scope) + "/" + saleId + "/checkout")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"expectedSaleVersion\":" + version + ",\"payments\":" + payments + "}"));
    }

    private int checkoutStatus(Scope scope, String token, String saleId, long version) throws Exception {
        return checkout(scope, token, saleId, version, "[]").andReturn().getResponse().getStatus();
    }

    private void seedBalance(RbacFixture fx, UUID branchId, String itemId, String quantity) {
        jdbc.update("insert into branch_inventory (id,organization_id,branch_id,inventory_item_id,quantity_on_hand,created_at,updated_at) "
                        + "values (?,?,?,?,?,now(),now())",
                UUID.randomUUID(), fx.orgId(), branchId, UUID.fromString(itemId), new BigDecimal(quantity));
    }

    private long saleVersion(String saleId) {
        return jdbc.queryForObject("select version from sales where id = ?", Long.class, UUID.fromString(saleId));
    }

    private BigDecimal quantity(UUID branchId, String itemId) {
        return jdbc.queryForObject("select quantity_on_hand from branch_inventory where branch_id = ? and inventory_item_id = ?",
                BigDecimal.class, branchId, UUID.fromString(itemId));
    }

    private String movementType(RbacFixture fx, UUID branchId, String itemId) {
        return jdbc.queryForObject("select type from inventory_movements where branch_id = ? and inventory_item_id = ?",
                String.class, branchId, UUID.fromString(itemId));
    }

    private BigDecimal movementDelta(RbacFixture fx, UUID branchId, String itemId) {
        return jdbc.queryForObject("select quantity_delta from inventory_movements where branch_id = ? and inventory_item_id = ?",
                BigDecimal.class, branchId, UUID.fromString(itemId));
    }

    private List<UUID> consumptionMovements(String saleId) {
        return jdbc.queryForList("select id from inventory_movements where sale_id = ? and type = 'SALE_CONSUMPTION'",
                UUID.class, UUID.fromString(saleId));
    }

    private long countMovements() {
        return jdbc.queryForObject("select count(*) from inventory_movements", Long.class);
    }

    private String id(String json) throws Exception {
        return objectMapper.readTree(json).get("id").asText();
    }

    private static String salesUrl(Scope scope) {
        return ORG + scope.fixture().orgId() + "/branches/" + scope.branchId() + "/cash-registers/"
                + scope.registerId() + "/cash-shifts/" + scope.shiftId() + "/sales";
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

    private record Scope(RbacFixture fixture, UUID branchId, String registerId, String shiftId) {}
}
