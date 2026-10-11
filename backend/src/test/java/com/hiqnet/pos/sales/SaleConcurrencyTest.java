package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
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

/**
 * Real PostgreSQL concurrency tests for the Sales aggregate and its coordination
 * with CashShift. The terminal invariant is that a CLOSED shift can never retain
 * an OPEN sale.
 */
class SaleConcurrencyTest extends AbstractRbacIntegrationTest {

    private static final int THREADS = 4;

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void createSaleAndCloseShiftRaceLeavesNoClosedShiftWithOpenSale() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "race-create-close");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String closeUrl = shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/close";

        List<Integer> statuses = runParallel(List.of(
                () -> responseStatus(post(salesUrl).header("Authorization", bearer(token))),
                () -> responseStatus(post(closeUrl).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":0}"))));

        assertThat(statuses).allMatch(status -> status == 200 || status == 201 || status == 409);
        assertThat(statuses).contains(409);
        assertThat(statuses.stream().filter(status -> status == 200 || status == 201).count()).isEqualTo(1);
        Long invalidState = jdbcTemplate.queryForObject(
                "select count(*) from sales s join cash_shifts cs on cs.id = s.cash_shift_id "
                        + "where s.cash_shift_id = ? and s.status = 'OPEN' and cs.status = 'CLOSED'",
                Long.class,
                UUID.fromString(shiftId));
        assertThat(invalidState).isZero();
    }

    @Test
    void cancelLastOpenSaleAndCloseShiftRaceReconcilesToCancelledSaleAndClosedShift() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "race-cancel-close");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        String closeUrl = shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/close";

        List<Integer> statuses = runParallel(List.of(
                () -> responseStatus(post(salesUrl + "/" + saleId + "/cancel").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0}")),
                () -> responseStatus(post(closeUrl).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"declaredCashAmount\":0}"))));

        assertThat(statuses).allMatch(status -> status == 200 || status == 409);
        assertThat(statuses).contains(200);
        UUID shiftUuid = UUID.fromString(shiftId);
        if (jdbcTemplate.queryForObject("select status from cash_shifts where id = ?", String.class, shiftUuid).equals("OPEN")) {
            assertThat(responseStatus(post(closeUrl).header("Authorization", bearer(token))
                    .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"declaredCashAmount\":0}"))).isEqualTo(200);
        }
        assertThat(jdbcTemplate.queryForObject("select status from sales where id = ?", String.class, UUID.fromString(saleId)))
                .isEqualTo("CANCELLED");
        assertThat(jdbcTemplate.queryForObject("select status from cash_shifts where id = ?", String.class, shiftUuid))
                .isEqualTo("CLOSED");
    }

    @Test
    void parallelSameVersionItemAddsHaveOneWinnerAndOneResultingLine() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "race-items");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "RACE-COFFEE", "Race Coffee", "7.2500");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        String addUrl = salesUrl + "/" + saleId + "/items";
        String body = "{\"expectedSaleVersion\":0,\"productId\":\"" + productId
                + "\",\"quantity\":2,\"modifierOptionIds\":[]}";

        List<Callable<Integer>> actions = new ArrayList<>();
        for (int index = 0; index < THREADS; index++) {
            actions.add(() -> responseStatus(post(addUrl).header("Authorization", bearer(token))
                    .contentType(MediaType.APPLICATION_JSON).content(body)));
        }
        List<Integer> statuses = runParallel(actions);

        assertThat(statuses.stream().filter(value -> value == 200).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(value -> value == 409).count()).isEqualTo(THREADS - 1);
        UUID saleUuid = UUID.fromString(saleId);
        Long activeLines = jdbcTemplate.queryForObject(
                "select count(*) from sale_items where sale_id = ? and status = 'ACTIVE'", Long.class, saleUuid);
        assertThat(activeLines).isEqualTo(1);
        assertThat(jdbcTemplate.queryForObject("select total_amount from sales where id = ?", java.math.BigDecimal.class, saleUuid))
                .isEqualByComparingTo("14.5000");
    }

    private UUID createBranch(RbacFixture fixture, String code) {
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        return branchRepository.saveAndFlush(Branch.create(organization, "Branch " + code, code, null)).getId();
    }

    private String createRegister(RbacFixture fixture, UUID branchId, String token) throws Exception {
        return id(post(registersUrl(fixture, branchId)).header("Authorization", bearer(token))
                .contentType(MediaType.APPLICATION_JSON).content("{\"code\":\"register\",\"name\":\"Register\"}"));
    }

    private String openShift(RbacFixture fixture, UUID branchId, String registerId, String token) throws Exception {
        return id(post(shiftsUrl(fixture, branchId, registerId)).header("Authorization", bearer(token))
                .contentType(MediaType.APPLICATION_JSON).content("{\"openingAmount\":0}"));
    }

    private String createProduct(RbacFixture fixture, String token, String sku, String name, String price) throws Exception {
        return id(post("/api/v1/organizations/" + fixture.orgId() + "/products").header("Authorization", bearer(token))
                .contentType(MediaType.APPLICATION_JSON)
                .content("{\"sku\":\"" + sku + "\",\"name\":\"" + name + "\",\"unitPrice\":" + price + "}"));
    }

    private int responseStatus(org.springframework.test.web.servlet.request.MockHttpServletRequestBuilder request) throws Exception {
        return mockMvc.perform(request).andReturn().getResponse().getStatus();
    }

    private String id(org.springframework.test.web.servlet.request.MockHttpServletRequestBuilder request) throws Exception {
        return objectMapper.readTree(mockMvc.perform(request).andExpect(status().isCreated()).andReturn().getResponse().getContentAsString())
                .get("id").asText();
    }

    private List<Integer> runParallel(List<Callable<Integer>> actions) throws Exception {
        CountDownLatch ready = new CountDownLatch(actions.size());
        CountDownLatch start = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(actions.size());
        try {
            List<Future<Integer>> futures = actions.stream().map(action -> pool.submit(() -> {
                ready.countDown();
                start.await();
                return action.call();
            })).toList();
            assertThat(ready.await(10, java.util.concurrent.TimeUnit.SECONDS)).isTrue();
            start.countDown();
            List<Integer> results = new ArrayList<>();
            for (Future<Integer> future : futures) {
                results.add(future.get());
            }
            return results;
        } finally {
            start.countDown();
            pool.shutdownNow();
        }
    }

    private static String registersUrl(RbacFixture fixture, UUID branchId) {
        return "/api/v1/organizations/" + fixture.orgId() + "/branches/" + branchId + "/cash-registers";
    }

    private static String shiftsUrl(RbacFixture fixture, UUID branchId, String registerId) {
        return registersUrl(fixture, branchId) + "/" + registerId + "/cash-shifts";
    }

    private static String salesUrl(RbacFixture fixture, UUID branchId, String registerId, String shiftId) {
        return shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/sales";
    }
}
