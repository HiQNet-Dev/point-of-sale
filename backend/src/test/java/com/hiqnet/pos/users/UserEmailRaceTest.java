package com.hiqnet.pos.users;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.application.UserService;
import com.hiqnet.pos.users.domain.User;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.stream.IntStream;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.test.context.ActiveProfiles;

/**
 * The database is the final authority on per-organization email uniqueness and
 * on email normalization. The existsBy pre-check is UX-only and cannot close
 * the concurrent race; PostgreSQL must.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
class UserEmailRaceTest {

    private static final String PASSWORD = "correct-horse-1";

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private UserService userService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void parallelCreateUserWithSameEmailYieldsExactlyOneWinner() {
        Organization org = organizationService.create("Org", "user-race-org", "UTC", "MXN", "es-MX");
        ExecutorService pool = Executors.newFixedThreadPool(4);
        try {
            CountDownLatch start = new CountDownLatch(1);
            List<Future<User>> futures = IntStream.range(0, 4)
                    .mapToObj(i -> pool.submit(() -> {
                        start.await();
                        return userService.create(org.getId(), "race-user@example.com", PASSWORD);
                    }))
                    .toList();
            start.countDown();

            int successes = 0;
            for (Future<User> future : futures) {
                try {
                    future.get();
                    successes++;
                } catch (ExecutionException e) {
                    Throwable cause = e.getCause();
                    assertTrue(
                            cause instanceof BusinessException
                                            && ((BusinessException) cause)
                                                    .getCode()
                                                    .equals(ErrorCodes.USER_EMAIL_ALREADY_EXISTS)
                                    || cause instanceof DataIntegrityViolationException,
                            "Expected a duplicate-email failure, got: " + cause);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new IllegalStateException(e);
                }
            }

            assertEquals(1, successes, "Exactly one concurrent create may win");
            Integer rows = jdbcTemplate.queryForObject(
                    "SELECT count(*) FROM users WHERE organization_id = ? AND email = 'race-user@example.com'",
                    Integer.class,
                    org.getId());
            assertEquals(1, rows);
        } finally {
            pool.shutdownNow();
        }
    }

    @Test
    void emailNormalizationConstraintIsEnforcedByTheDatabase() {
        Organization org = organizationService.create("Org", "user-check-org", "UTC", "MXN", "es-MX");

        DataIntegrityViolationException violation = assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        "INSERT INTO users (id, organization_id, email, password_hash, created_at, updated_at) "
                                + "VALUES (?, ?, ?, ?, ?, ?)",
                        UUID.randomUUID(), org.getId(), "Mixed@Example.com", "hash",
                        Timestamp.from(Instant.now()), Timestamp.from(Instant.now())));

        assertInstanceOf(DataIntegrityViolationException.class, violation);
    }
}
