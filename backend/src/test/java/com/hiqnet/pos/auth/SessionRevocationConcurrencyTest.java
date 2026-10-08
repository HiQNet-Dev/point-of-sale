package com.hiqnet.pos.auth;

import static org.assertj.core.api.Assertions.assertThat;

import com.hiqnet.pos.auth.domain.RefreshSession;
import com.hiqnet.pos.auth.domain.RefreshSessionRepository;
import com.hiqnet.pos.auth.infrastructure.RefreshTokenHasher;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.users.domain.User;
import java.time.Instant;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;

/**
 * Real PostgreSQL races between session mutations of the SAME user. The user
 * row is the coordination lock (PostgreSQL as authority), so the invariant
 * "no live successor survives a revocation" holds regardless of interleaving.
 * Assertions check the final DATABASE state, not only the HTTP result.
 */
class SessionRevocationConcurrencyTest extends AbstractAuthIntegrationTest {

    @Autowired
    private RefreshSessionRepository refreshSessionRepository;

    @Test
    void concurrentRefreshAndLogoutLeaveNoLiveSession() throws Exception {
        String orgCode = createOrg();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("r-logout@"), PASSWORD);
        String token = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);
        UUID familyId = familyIdOf(token);

        race(
                () -> swallow(() -> authService.refresh(token)),
                () -> authService.logout(token));

        assertNoLiveSessionInFamily(familyId);
    }

    @Test
    void concurrentRefreshAndLogoutAllLeaveNoLiveSession() throws Exception {
        String orgCode = createOrg();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("r-all@"), PASSWORD);
        String token = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);

        race(
                () -> swallow(() -> authService.refresh(token)),
                () -> authService.logoutAll(user.getId()));

        List<RefreshSession> sessions = refreshSessionRepository.findByUserId(user.getId());
        assertThat(sessions).isNotEmpty();
        assertThat(sessions).allMatch(session -> !session.isReusable(Instant.now()));
    }

    @Test
    void concurrentReuseRevocationAndRefreshLeaveNoLiveSession() throws Exception {
        String orgCode = createOrg();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("reuse-race@"), PASSWORD);
        String tokenA = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);
        String tokenB = refreshRawToken(tokenA);
        UUID familyId = familyIdOf(tokenB);

        race(
                // Presenting the consumed token A triggers reuse -> family revocation.
                () -> swallow(() -> authService.refresh(tokenA)),
                () -> swallow(() -> authService.refresh(tokenB)));

        assertNoLiveSessionInFamily(familyId);
    }

    private void assertNoLiveSessionInFamily(UUID familyId) {
        List<RefreshSession> family = refreshSessionRepository.findByFamilyId(familyId);
        assertThat(family).isNotEmpty();
        assertThat(family).allMatch(session -> !session.isReusable(Instant.now()));
    }

    private UUID familyIdOf(String rawToken) {
        return refreshSessionRepository
                .findByTokenHash(RefreshTokenHasher.hash(rawToken))
                .orElseThrow()
                .getFamilyId();
    }

    private static void race(Runnable first, Runnable second) throws Exception {
        ExecutorService pool = Executors.newFixedThreadPool(2);
        CountDownLatch start = new CountDownLatch(1);
        try {
            Future<?> f1 = pool.submit(() -> runAfter(start, first));
            Future<?> f2 = pool.submit(() -> runAfter(start, second));
            start.countDown();
            f1.get(30, TimeUnit.SECONDS);
            f2.get(30, TimeUnit.SECONDS);
        } finally {
            start.countDown();
            pool.shutdownNow();
        }
    }

    private static void runAfter(CountDownLatch start, Runnable action) {
        try {
            if (!start.await(10, TimeUnit.SECONDS)) {
                throw new IllegalStateException("start timeout");
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(e);
        }
        action.run();
    }

    private static void swallow(Runnable action) {
        try {
            action.run();
        } catch (BusinessException expected) {
            // The loser of the race is expected to fail with a public error.
        }
    }

    private static String newOrgCode() {
        return "org-" + UUID.randomUUID().toString().substring(0, 8);
    }

    private String createOrg() {
        String code = newOrgCode();
        createOrganizationWithCode(code);
        return code;
    }

    private String orgIdOf(String code) {
        return organizationRepository.findByCode(code).orElseThrow().getId().toString();
    }
}
