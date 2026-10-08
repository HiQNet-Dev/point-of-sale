package com.hiqnet.pos;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
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
 * The database is the final authority on uniqueness: the existsBy pre-checks in
 * the services are UX-only and cannot close the concurrent race. These tests
 * prove the unique constraints exist and are enforced by PostgreSQL, and that
 * concurrent duplicate creates converge to exactly one winner with the stable
 * 409 duplicate-code error (either via constraint translation or pre-check).
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
class DuplicateConstraintDbTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private BranchService branchService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void organizationCodeConstraintIsEnforcedByTheDatabase() {
        Organization existing = organizationService.create("Org", "constraint-org", "UTC", "MXN", "es-MX");

        DataIntegrityViolationException violation = assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        "INSERT INTO organizations (id, name, code, timezone, currency, locale, status, version, created_at, updated_at) "
                                + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        UUID.randomUUID(), "Other Org", existing.getCode(), "UTC", "MXN", "es-MX",
                        "ACTIVE", 0, Timestamp.from(Instant.now()), Timestamp.from(Instant.now())));

        assertInstanceOf(DataIntegrityViolationException.class, violation);
    }

    @Test
    void branchOrganizationCodeConstraintIsEnforcedByTheDatabase() {
        Organization organization = organizationService.create("Org", "constraint-branch-org", "UTC", "MXN", "es-MX");
        Branch existing = branchService.create(organization.getId(), "Sucursal", "centro", null);

        DataIntegrityViolationException violation = assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        "INSERT INTO branches (id, organization_id, name, code, timezone, status, version, created_at, updated_at) "
                                + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        UUID.randomUUID(), organization.getId(), "Otra", existing.getCode(), null,
                        "ACTIVE", 0, Timestamp.from(Instant.now()), Timestamp.from(Instant.now())));

        assertInstanceOf(DataIntegrityViolationException.class, violation);
    }

    @Test
    void parallelCreateOrganizationWithSameCodeYieldsExactlyOneWinner() {
        ExecutorService pool = Executors.newFixedThreadPool(4);
        try {
            CountDownLatch start = new CountDownLatch(1);
            List<Future<Organization>> futures = IntStream.range(0, 4)
                    .mapToObj(i -> pool.submit(() -> {
                        start.await();
                        return organizationService.create("Cafeteria Race", "race-constraint", "UTC", "MXN", "es-MX");
                    }))
                    .toList();
            start.countDown();

            int successes = 0;
            for (Future<Organization> future : futures) {
                try {
                    future.get();
                    successes++;
                } catch (ExecutionException e) {
                    Throwable cause = e.getCause();
                    assertTrue(
                            cause instanceof BusinessException
                                            && ((BusinessException) cause).getCode()
                                                    .equals(ErrorCodes.ORGANIZATION_CODE_ALREADY_EXISTS)
                                    || cause instanceof DataIntegrityViolationException,
                            "Expected a duplicate-code failure, got: " + cause);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new IllegalStateException(e);
                }
            }

            assertEquals(1, successes, "Exactly one concurrent create may win");
            Integer rows = jdbcTemplate.queryForObject(
                    "SELECT count(*) FROM organizations WHERE code = 'race-constraint'", Integer.class);
            assertEquals(1, rows);
        } finally {
            pool.shutdownNow();
        }
    }

    @Test
    void parallelCreateBranchWithSameCodeYieldsExactlyOneWinner() {
        Organization org = organizationService.create("Org", "constraint-branch-race", "UTC", "MXN", "es-MX");
        ExecutorService pool = Executors.newFixedThreadPool(4);
        try {
            CountDownLatch start = new CountDownLatch(1);
            List<Future<Branch>> futures = IntStream.range(0, 4)
                    .mapToObj(i -> pool.submit(() -> {
                        start.await();
                        return branchService.create(org.getId(), "Sucursal Race", "race-centro", null);
                    }))
                    .toList();
            start.countDown();

            int successes = 0;
            for (Future<Branch> future : futures) {
                try {
                    future.get();
                    successes++;
                } catch (ExecutionException e) {
                    Throwable cause = e.getCause();
                    assertTrue(
                            cause instanceof BusinessException
                                            && ((BusinessException) cause).getCode().equals(ErrorCodes.BRANCH_CODE_ALREADY_EXISTS)
                                    || cause instanceof DataIntegrityViolationException,
                            "Expected a duplicate-code failure, got: " + cause);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new IllegalStateException(e);
                }
            }

            assertEquals(1, successes, "Exactly one concurrent create may win");
            Integer rows = jdbcTemplate.queryForObject(
                    "SELECT count(*) FROM branches WHERE organization_id = ? AND code = 'race-centro'",
                    Integer.class, org.getId());
            assertEquals(1, rows);
        } finally {
            pool.shutdownNow();
        }
    }
}