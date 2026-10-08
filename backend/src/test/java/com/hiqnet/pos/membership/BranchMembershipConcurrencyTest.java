package com.hiqnet.pos.membership;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.users.domain.User;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Concurrency: N parallel create requests for the SAME (organization, branch,
 * user) must produce EXACTLY ONE membership; every other request fails with
 * 409 (the pre-check or, under the race, the database unique constraint
 * translated by {@code ConstraintTranslator}). PostgreSQL is the final
 * authority, never Java.
 */
class BranchMembershipConcurrencyTest extends AbstractRbacIntegrationTest {

    private static final int THREADS = 4;

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Test
    void parallelCreatesYieldExactlyOneMembership() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(
                Branch.create(organization, "Branch race", "race-" + UUID.randomUUID().toString().substring(0, 8), null));
        User member = User.create(fx.orgId(), uniqueEmail("race@"), passwordEncoder.encode(PASSWORD));
        userRepository.saveAndFlush(member);

        String url = "/api/v1/organizations/" + fx.orgId() + "/branches/" + branch.getId()
                + "/memberships";
        String body = "{\"userId\":\"" + member.getId() + "\"}";

        CountDownLatch ready = new CountDownLatch(THREADS);
        CountDownLatch start = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(THREADS);
        List<Future<Integer>> results = new ArrayList<>();
        for (int i = 0; i < THREADS; i++) {
            results.add(pool.submit(() -> {
                ready.countDown();
                start.await();
                MvcResult result = mockMvc.perform(post(url)
                                .header("Authorization", bearer(token))
                                .contentType(MediaType.APPLICATION_JSON)
                                .content(body))
                        .andReturn();
                return result.getResponse().getStatus();
            }));
        }
        ready.await();
        start.countDown();

        List<Integer> statuses = new ArrayList<>();
        for (Future<Integer> future : results) {
            statuses.add(future.get());
        }
        pool.shutdown();

        assertThat(statuses.stream().filter(s -> s == 201).count()).isEqualTo(1);
        assertThat(statuses.stream().filter(s -> s == 409).count()).isEqualTo(THREADS - 1);

        long stored = membershipRepository
                .findByOrganizationIdAndBranchId(
                        fx.orgId(), branch.getId(), PageRequest.of(0, 100, Sort.by("createdAt")))
                .getTotalElements();
        assertThat(stored).isEqualTo(1);
    }
}