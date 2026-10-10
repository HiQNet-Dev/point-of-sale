package com.hiqnet.pos.catalog;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

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

/**
 * Concurrency: the database (unique indexes) and the product's optimistic
 * version are the final authority. N parallel same-code group creates yield one
 * 201; N parallel replacements of the same product+version yield one 200.
 */
class ModifierConcurrencyTest extends AbstractRbacIntegrationTest {

    private static final int THREADS = 4;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private String createGroup(String organizationId, String token, String code) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"Group\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createProduct(String organizationId, String token, String sku) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"" + sku + "\",\"name\":\"" + sku + "\",\"unitPrice\":10.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void parallelGroupCreatesYieldExactlyOne() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String url = "/api/v1/organizations/" + fx.orgId() + "/modifier-groups";
        String body = "{\"code\":\"milk\",\"name\":\"Milk\"}";

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
                "select count(*) from modifier_groups where organization_id = ? and code = 'milk'",
                Long.class,
                fx.orgId());
        assertThat(stored).isEqualTo(1);
    }

    @Test
    void parallelReplacesOfSameVersionYieldExactlyOne() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        String url = "/api/v1/organizations/" + fx.orgId() + "/products/" + productId + "/modifier-groups";
        String body = "{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}";

        List<Integer> statuses = runParallel(THREADS, () -> mockMvc.perform(put(url)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andReturn()
                .getResponse()
                .getStatus());

        assertThat(statuses.stream().filter(s -> s == 200).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(s -> s == 409).count()).isEqualTo(THREADS - 1);
    }

    private List<Integer> runParallel(int threads, java.util.concurrent.Callable<Integer> action) throws Exception {
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
