package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

class SupplierConcurrencyTest extends AbstractRbacIntegrationTest {

    private static final int THREADS = 4;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void parallelCreatesOfTheSameCodeYieldOneCreatedAndControlledConflicts() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        String url = "/api/v1/organizations/" + fixture.orgId() + "/suppliers";
        String body = "{\"code\":\"supplier\",\"name\":\"Supplier\"}";

        List<Integer> statuses = runParallel(THREADS, () -> mockMvc.perform(post(url)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andReturn()
                .getResponse()
                .getStatus());

        assertThat(statuses.stream().filter(status -> status == 201).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(status -> status == 409).count()).isEqualTo(THREADS - 1);
        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from suppliers where organization_id = ? and code = 'supplier'", Long.class, fixture.orgId());
        assertThat(stored).isEqualTo(1);
    }

    private List<Integer> runParallel(int threads, java.util.concurrent.Callable<Integer> action) throws Exception {
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
