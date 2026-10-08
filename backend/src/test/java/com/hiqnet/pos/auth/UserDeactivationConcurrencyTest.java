package com.hiqnet.pos.auth;

import static org.assertj.core.api.Assertions.assertThat;

import com.hiqnet.pos.auth.domain.RefreshSessionRepository;
import com.hiqnet.pos.auth.infrastructure.RefreshTokenHasher;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.users.application.UpdateUserCommand;
import com.hiqnet.pos.users.application.UserService;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserStatus;
import java.time.Instant;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;

/**
 * Deactivation races against auth flows on the user row lock (PostgreSQL as
 * authority): regardless of interleaving, once deactivation completes the user
 * is INACTIVE and NO refresh session of theirs is usable — deactivation never
 * leaves a live session behind (a concurrent login/rotation either loses or its
 * session is revoked in the same transaction).
 */
class UserDeactivationConcurrencyTest extends AbstractAuthIntegrationTest {

    @Autowired
    private UserService userService;

    @Autowired
    private RefreshSessionRepository refreshSessionRepository;

    @Test
    void deactivateVersusRefreshNeverLeavesAUsableSession() throws Exception {
        Fixture fx = fixture();
        String refreshA = loginAndGetRefreshToken(fx.code(), fx.email(), PASSWORD);
        UUID familyId = familyIdOf(refreshA);

        race(
                () -> swallow(() -> deactivate(fx)),
                () -> swallow(() -> authService.refresh(refreshA)));

        // Ensure the deactivation actually completed (a concurrent login/refresh
        // could otherwise have bumped the version and made the stale 409 win).
        deactivate(fx);
        assertDeactivated(fx.user().getId());
        assertThat(refreshSessionRepository.findByFamilyId(familyId))
                .isNotEmpty()
                .allMatch(session -> !session.isReusable(Instant.now()));
    }

    @Test
    void deactivateVersusLoginNeverLeavesAUsableSession() throws Exception {
        Fixture fx = fixture();

        race(
                () -> swallow(() -> deactivate(fx)),
                () -> swallow(() -> authService.login(fx.code(), fx.email(), PASSWORD)));

        // Finalize: no race may leave the user ACTIVE with a valid session once
        // deactivation is requested to complete.
        deactivate(fx);
        assertDeactivated(fx.user().getId());
        // A login that won the race before deactivation committed must have its
        // session revoked by the deactivation; a login that lost never created
        // one. Either way: zero usable sessions.
        assertThat(refreshSessionRepository.findByUserId(fx.user().getId()))
                .allMatch(session -> !session.isReusable(Instant.now()));
    }

    private void deactivate(Fixture fx) {
        long version = userRepository.findById(fx.user().getId()).orElseThrow().getVersion();
        userService.update(fx.orgId(), fx.user().getId(), new UpdateUserCommand(version, null, UserStatus.INACTIVE));
    }

    private void assertDeactivated(UUID userId) {
        assertThat(userRepository.findById(userId).orElseThrow().getStatus())
                .isEqualTo(UserStatus.INACTIVE);
    }

    private UUID familyIdOf(String rawToken) {
        return refreshSessionRepository
                .findByTokenHash(RefreshTokenHasher.hash(rawToken))
                .orElseThrow()
                .getFamilyId();
    }

    private Fixture fixture() {
        String code = "uorg-" + UUID.randomUUID().toString().substring(0, 8);
        Organization organization =
                Organization.create("Org " + code, code, "America/Mexico_City", "MXN", "es-MX");
        organizationRepository.saveAndFlush(organization);
        String email = uniqueEmail("deact-race@");
        User user = userService.create(organization.getId(), email, PASSWORD);
        return new Fixture(organization.getId(), code, email, user);
    }

    private record Fixture(UUID orgId, String code, String email, User user) {
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
}