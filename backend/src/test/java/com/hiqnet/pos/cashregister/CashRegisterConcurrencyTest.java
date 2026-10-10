package com.hiqnet.pos.cashregister;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
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
 * Concurrency: the database unique index and the register's optimistic version
 * are the final authority. N parallel same-code creates in one branch yield one
 * 201; N parallel same-version patches yield one 200.
 */
class CashRegisterConcurrencyTest extends AbstractRbacIntegrationTest {

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

    private static String url(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }

    @Test
    void parallelCreateWithSameCodeYieldsExactlyOne() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String url = url(fx, branchId);
        String body = "{\"code\":\"register-1\",\"name\":\"Front\"}";

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
                "select count(*) from cash_registers where branch_id = ? and code = 'register-1'",
                Long.class,
                branchId);
        assertThat(stored).isEqualTo(1);
    }

    @Test
    void parallelPatchOfSameVersionYieldsExactlyOne() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String response = mockMvc.perform(post(url(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        String registerId = objectMapper.readTree(response).get("id").asText();
        String url = url(fx, branchId) + "/" + registerId;
        String body = "{\"expectedVersion\":0,\"name\":\"Front 2\"}";

        List<Integer> statuses = runParallel(THREADS, () -> mockMvc.perform(patch(url)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andReturn()
                .getResponse()
                .getStatus());

        assertThat(statuses.stream().filter(s -> s == 200).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(s -> s == 409).count()).isEqualTo(THREADS - 1);
        Long version = jdbcTemplate.queryForObject(
                "select version from cash_registers where id = ?", Long.class, UUID.fromString(registerId));
        String storedName = jdbcTemplate.queryForObject(
                "select name from cash_registers where id = ?", String.class, UUID.fromString(registerId));
        assertThat(version).isEqualTo(1);
        assertThat(storedName).isEqualTo("Front 2");
    }

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
