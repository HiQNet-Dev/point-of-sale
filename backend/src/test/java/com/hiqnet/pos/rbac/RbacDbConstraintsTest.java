package com.hiqnet.pos.rbac;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.rbac.application.RoleService;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.stream.IntStream;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * PostgreSQL is the tenant authority: a User of organization A can never be
 * assigned a Role of organization B (composite FKs), and the role code
 * uniqueness race has exactly one winner.
 */
class RbacDbConstraintsTest extends AbstractRbacIntegrationTest {

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Autowired
    private RoleService roleService;

    @Test
    void crossTenantRoleAssignmentIsImpossibleAtDatabaseLevel() {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        Role roleB = Role.create(fxB.orgId(), "role-b", "Role B");
        roleRepository.saveAndFlush(roleB);

        DataIntegrityViolationException violation = assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        "INSERT INTO user_organization_roles (organization_id, user_id, role_id, created_at) "
                                + "VALUES (?, ?, ?, ?)",
                        fxA.orgId(), fxA.userId(), roleB.getId(), Timestamp.from(Instant.now())));

        assertInstanceOf(DataIntegrityViolationException.class, violation);
        Integer rows = jdbcTemplate.queryForObject(
                "SELECT count(*) FROM user_organization_roles WHERE user_id = ?", Integer.class, fxA.userId());
        assertEquals(0, rows);
    }

    @Test
    void crossTenantUserAssignmentIsImpossibleAtDatabaseLevel() {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        Role roleA = Role.create(fxA.orgId(), "role-a", "Role A");
        roleRepository.saveAndFlush(roleA);

        // fxB.userId belongs to B; the user composite FK rejects it under org A.
        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        "INSERT INTO user_organization_roles (organization_id, user_id, role_id, created_at) "
                                + "VALUES (?, ?, ?, ?)",
                        fxA.orgId(), fxB.userId(), roleA.getId(), Timestamp.from(Instant.now())));
    }

    @Test
    void parallelCreateRoleWithSameCodeYieldsExactlyOneWinner() {
        RbacFixture fx = fixture();
        ExecutorService pool = Executors.newFixedThreadPool(4);
        try {
            CountDownLatch start = new CountDownLatch(1);
            List<Future<Role>> futures = IntStream.range(0, 4)
                    .mapToObj(i -> pool.submit(() -> {
                        start.await();
                        return roleService.create(fx.orgId(), "race-role", "Race Role");
                    }))
                    .toList();
            start.countDown();

            int successes = 0;
            for (Future<Role> future : futures) {
                try {
                    future.get();
                    successes++;
                } catch (ExecutionException e) {
                    Throwable cause = e.getCause();
                    assertTrue(
                            cause instanceof BusinessException
                                            && ((BusinessException) cause)
                                                    .getCode()
                                                    .equals(ErrorCodes.ROLE_CODE_ALREADY_EXISTS)
                                    || cause instanceof DataIntegrityViolationException,
                            "Expected a duplicate-role failure, got: " + cause);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new IllegalStateException(e);
                }
            }

            assertEquals(1, successes, "Exactly one concurrent create may win");
            Integer rows = jdbcTemplate.queryForObject(
                    "SELECT count(*) FROM roles WHERE organization_id = ? AND code = 'race-role'",
                    Integer.class,
                    fx.orgId());
            assertEquals(1, rows);
        } finally {
            pool.shutdownNow();
        }
    }
}