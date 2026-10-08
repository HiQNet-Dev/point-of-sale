package com.hiqnet.pos.auth;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.auth.api.AuthResponses.TokenResponse;
import com.hiqnet.pos.auth.application.AuthService;
import com.hiqnet.pos.auth.domain.RefreshSession;
import com.hiqnet.pos.auth.domain.RefreshSessionRepository;
import com.hiqnet.pos.auth.infrastructure.RefreshTokenHasher;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.users.domain.User;
import java.time.Instant;
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

/**
 * Refresh concurrency: two threads racing to use the SAME refresh token must
 * yield exactly one winner; the loser observes the rotated state and revokes
 * the family (PESSIMISTIC_WRITE serializes on the row). Two independent valid
 * tokens derived from token A are impossible.
 */
class RefreshConcurrencyTest extends AbstractAuthIntegrationTest {

    @Autowired
    private RefreshSessionRepository refreshSessionRepository;

    @Test
    void concurrentRefreshOfSameTokenYieldsExactlyOneWinner() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("race@"), PASSWORD);
        String tokenA = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);

        ExecutorService pool = Executors.newFixedThreadPool(2);
        CountDownLatch start = new CountDownLatch(1);
        try {
            Callable<String> attempt = () -> {
                if (!start.await(10, TimeUnit.SECONDS)) {
                    throw new IllegalStateException("start timeout");
                }
                try {
                    TokenResponse response = authService.refresh(tokenA);
                    return response.refreshToken();
                } catch (BusinessException e) {
                    return null;
                }
            };
            Future<String> future1 = pool.submit(attempt);
            Future<String> future2 = pool.submit(attempt);
            start.countDown();

            String winner = null;
            int failures = 0;
            for (Future<String> future : new Future[] {future1, future2}) {
                String result = future.get(20, TimeUnit.SECONDS);
                if (result == null) {
                    failures++;
                } else {
                    winner = result;
                }
            }

            assertThat(failures).isEqualTo(1);
            assertThat(winner).isNotNull();

            // The loser detected reuse and revoked the family: the winner's
            // own token is now dead too.
            mockMvc.perform(post("/api/v1/auth/refresh")
                            .contentType(MediaType.APPLICATION_JSON)
                            .content("""
                                    {"refreshToken":"%s"}
                                    """.formatted(winner)))
                    .andExpect(status().isUnauthorized());

            // Database check: no session of the family is reusable, so two
            // valid successors are impossible.
            UUID familyId = refreshSessionRepository
                    .findByTokenHash(RefreshTokenHasher.hash(tokenA))
                    .orElseThrow()
                    .getFamilyId();
            List<RefreshSession> family = refreshSessionRepository.findByFamilyId(familyId);
            assertThat(family).isNotEmpty();
            assertThat(family).allMatch(session -> !session.isReusable(Instant.now()));
        } finally {
            start.countDown();
            pool.shutdownNow();
        }
    }

    private String createOrganizationAndGetCode() {
        String code = "org-" + UUID.randomUUID().toString().substring(0, 8);
        createOrganizationWithCode(code);
        return code;
    }

    private String orgIdOf(String code) {
        return organizationRepository.findByCode(code).orElseThrow().getId().toString();
    }
}