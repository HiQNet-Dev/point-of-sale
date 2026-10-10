package com.hiqnet.pos.bootstrap;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.mock.web.MockHttpServletResponse;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Two concurrent bootstrap requests with the SAME valid secret serialize on the
 * PostgreSQL singleton row ({@code SELECT ... FOR UPDATE}): exactly one wins
 * (201) and the other observes COMPLETED (409 BOOTSTRAP_ALREADY_COMPLETED).
 * No JVM lock — PostgreSQL is the coordinator. Real concurrency, no mocks.
 */
class BootstrapConcurrencyTest extends AbstractBootstrapIntegrationTest {

    private static final int WORKERS = 2;

    @Test
    void concurrentBootstrapRunsProduceExactlyOneWinner() throws Exception {
        ExecutorService pool = Executors.newFixedThreadPool(WORKERS);
        try {
            CountDownLatch ready = new CountDownLatch(WORKERS);
            CountDownLatch go = new CountDownLatch(1);
            List<Future<HttpOutcome>> futures = java.util.stream.IntStream.range(0, WORKERS)
                    .mapToObj(i -> pool.submit(() -> runBootstrap(ready, go, i)))
                    .toList();
            assertThat(ready.await(10, TimeUnit.SECONDS)).isTrue();
            go.countDown();

            List<HttpOutcome> outcomes = futures.stream()
                    .map(future -> {
                        try {
                            return future.get(30, TimeUnit.SECONDS);
                        } catch (Exception e) {
                            throw new IllegalStateException(e);
                        }
                    })
                    .toList();

            assertThat(outcomes).hasSize(WORKERS);
            assertThat(outcomes.stream().filter(o -> o.status() == 201)).hasSize(1);
            assertThat(outcomes.stream().filter(o -> o.status() == 409)).hasSize(1);
            assertThat(outcomes.stream()
                            .filter(o -> o.status() == 409)
                            .allMatch(o -> o.errorCode().equals("BOOTSTRAP_ALREADY_COMPLETED")))
                    .isTrue();

            // Exactly one installation topology persisted.
            Long organizations = jdbcTemplate.queryForObject("select count(*) from organizations", Long.class);
            assertThat(organizations).isEqualTo(1);
            Long users = jdbcTemplate.queryForObject("select count(*) from users", Long.class);
            assertThat(users).isEqualTo(1);
            Long roles = jdbcTemplate.queryForObject("select count(*) from roles", Long.class);
            assertThat(roles).isEqualTo(1);
            assertThat(installationCompleted()).isTrue();
        } finally {
            pool.shutdownNow();
        }
    }

    private HttpOutcome runBootstrap(CountDownLatch ready, CountDownLatch go, int index) throws Exception {
        ready.countDown();
        go.await(10, TimeUnit.SECONDS);
        // Unique request bodies: the loser must fail from the DB state, not
        // from a client-side duplicate check.
        String code = "worker-org-" + index;
        String email = uniqueEmail("worker" + index + "@");
        MockMvc mvc = mockMvc;
        MvcResult result = mvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody(code, email, PASSWORD)))
                .andReturn();
        MockHttpServletResponse response = result.getResponse();
        String errorCode = null;
        if (response.getContentAsString() != null && !response.getContentAsString().isBlank()) {
            try {
                errorCode = objectMapper.readTree(response.getContentAsString()).path("code").asText(null);
            } catch (Exception ignored) {
                // Non-JSON body: leave the code unknown.
            }
        }
        return new HttpOutcome(response.getStatus(), errorCode);
    }

    private record HttpOutcome(int status, String errorCode) {}
}