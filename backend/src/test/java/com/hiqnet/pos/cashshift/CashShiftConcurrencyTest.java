package com.hiqnet.pos.cashshift;

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
 * Concurrency: the partial unique index is the final authority on "one OPEN
 * shift per register" and the shift's optimistic version guards close. Parallel
 * same-register opens yield one 201; parallel same-version closes yield one 200.
 */
class CashShiftConcurrencyTest extends AbstractRbacIntegrationTest {

    private static final int THREADS = 4;

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private UUID createBranch(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository
                .saveAndFlush(Branch.create(organization, "Branch " + code, code, null))
                .getId();
    }

    private String createRegister(RbacFixture fx, UUID branchId, String token, String code) throws Exception {
        String response = mockMvc.perform(post(registersUrl(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + code + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private static String registersUrl(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }

    private static String shiftsUrl(RbacFixture fx, UUID branchId, String registerId) {
        return registersUrl(fx, branchId) + "/" + registerId + "/cash-shifts";
    }

    @Test
    void parallelOpenOnSameRegisterYieldsExactlyOne() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String url = shiftsUrl(fx, branchId, registerId);
        String body = "{\"openingAmount\":100.0000}";

        List<Integer> statuses = runParallel(THREADS, () -> mockMvc.perform(post(url)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andReturn()
                .getResponse()
                .getStatus());

        assertThat(statuses.stream().filter(s -> s == 201).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(s -> s == 409).count()).isEqualTo(THREADS - 1);
        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from cash_shifts where cash_register_id = ? and status = 'OPEN'",
                Long.class,
                UUID.fromString(registerId));
        assertThat(stored).isEqualTo(1);
    }

    @Test
    void parallelOpenOnDifferentRegistersAllSucceed() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        List<String> registers = new ArrayList<>();
        for (int i = 0; i < THREADS; i++) {
            registers.add(createRegister(fx, branchId, token, "register-" + i));
        }

        List<Integer> statuses = runParallel(THREADS, () -> {
            String registerId = registers.get(index.getAndIncrement() % registers.size());
            return mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                            .header("Authorization", bearer(token))
                            .contentType(MediaType.APPLICATION_JSON)
                            .content("{\"openingAmount\":100.0000}"))
                    .andReturn()
                    .getResponse()
                    .getStatus();
        });

        assertThat(statuses).containsOnly(201);
        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from cash_shifts where organization_id = ? and status = 'OPEN'",
                Long.class,
                fx.orgId());
        assertThat(stored).isEqualTo(THREADS);
    }

    @Test
    void parallelCloseOfSameVersionYieldsExactlyOne() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String response = mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        String shiftId = objectMapper.readTree(response).get("id").asText();
        String url = shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close";
        String body = "{\"expectedVersion\":0,\"declaredCashAmount\":100.0000}";

        List<Integer> statuses = runParallel(THREADS, () -> mockMvc.perform(post(url)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andReturn()
                .getResponse()
                .getStatus());

        assertThat(statuses.stream().filter(s -> s == 200).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(s -> s == 409).count()).isEqualTo(THREADS - 1);
        String status = jdbcTemplate.queryForObject(
                "select status from cash_shifts where id = ?", String.class, UUID.fromString(shiftId));
        assertThat(status).isEqualTo("CLOSED");
    }

    private final java.util.concurrent.atomic.AtomicInteger index = new java.util.concurrent.atomic.AtomicInteger();

    private List<Integer> runParallel(int threads, Callable<Integer> action) throws Exception {
        CountDownLatch ready = new CountDownLatch(threads);
        CountDownLatch start = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(threads);
        List<Future<Integer>> results = new ArrayList<>();
        for (int i = 0; i < threads; i++) {
            results.add(pool.submit(() -> {
                ready.countDown();
                start.await();
                return action.call();
            }));
        }
        ready.await();
        start.countDown();
        List<Integer> statuses = new ArrayList<>();
        for (Future<Integer> future : results) {
            statuses.add(future.get());
        }
        pool.shutdown();
        return statuses;
    }
}
