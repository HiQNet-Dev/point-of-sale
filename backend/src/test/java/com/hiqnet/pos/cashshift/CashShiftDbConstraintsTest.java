package com.hiqnet.pos.cashshift;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
import java.math.BigDecimal;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.test.context.ActiveProfiles;

/**
 * The database is the final authority on cash-shift integrity: the partial unique
 * index keeps at most one OPEN shift per register, the composite FKs keep a shift
 * in the SAME organization as its register and its actors, and the CHECKs enforce
 * state consistency and non-negative amounts.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
class CashShiftDbConstraintsTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private BranchService branchService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private static final String INSERT_REGISTER =
            "INSERT INTO cash_registers (id, organization_id, branch_id, code, name, status, version, "
                    + "created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static final String INSERT_SHIFT =
            "INSERT INTO cash_shifts (id, organization_id, branch_id, cash_register_id, opened_by_user_id, "
                    + "closed_by_user_id, status, opening_amount, closing_declared_amount, opened_at, closed_at, "
                    + "version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private UUID insertRegister(UUID orgId, UUID branchId, String code) {
        UUID registerId = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(INSERT_REGISTER, registerId, orgId, branchId, code, "Front", "ACTIVE", 0, now, now);
        return registerId;
    }

    private Object[] openShiftRow(UUID orgId, UUID branchId, UUID registerId, UUID userId, String amount) {
        Timestamp now = Timestamp.from(Instant.now());
        return new Object[] {
            UUID.randomUUID(), orgId, branchId, registerId, userId, null, "OPEN",
            new BigDecimal(amount), null, now, null, 0, now, now
        };
    }

    @Test
    void secondOpenShiftOnSameRegisterIsRejected() {
        Organization org = organizationService.create("Org", "cs-open", "UTC", "MXN", "es-MX");
        UUID branchId = branchService.create(org.getId(), "Centro", "centro", null).getId();
        UUID registerId = insertRegister(org.getId(), branchId, "register-1");
        UUID userId = insertUser(org.getId());

        jdbcTemplate.update(INSERT_SHIFT, openShiftRow(org.getId(), branchId, registerId, userId, "10.0000"));

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_SHIFT, openShiftRow(org.getId(), branchId, registerId, userId, "20.0000")));
    }

    @Test
    void closedShiftAllowsANewOpenShiftOnSameRegister() {
        Organization org = organizationService.create("Org", "cs-reopen", "UTC", "MXN", "es-MX");
        UUID branchId = branchService.create(org.getId(), "Centro", "centro", null).getId();
        UUID registerId = insertRegister(org.getId(), branchId, "register-1");
        UUID userId = insertUser(org.getId());

        UUID firstId = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(
                INSERT_SHIFT,
                firstId, org.getId(), branchId, registerId, userId, userId, "CLOSED",
                new BigDecimal("10.0000"), new BigDecimal("10.0000"), now, now, 1, now, now);

        assertDoesNotThrow(() -> jdbcTemplate.update(
                INSERT_SHIFT, openShiftRow(org.getId(), branchId, registerId, userId, "5.0000")));
    }

    @Test
    void shiftCannotReferenceRegisterOfAnotherOrganization() {
        Organization orgA = organizationService.create("Org A", "cs-fk-a", "UTC", "MXN", "es-MX");
        Organization orgB = organizationService.create("Org B", "cs-fk-b", "UTC", "MXN", "es-MX");
        UUID branchA = branchService.create(orgA.getId(), "Centro", "centro", null).getId();
        UUID registerA = insertRegister(orgA.getId(), branchA, "register-1");
        UUID userB = insertUser(orgB.getId());

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_SHIFT, openShiftRow(orgB.getId(), branchA, registerA, userB, "10.0000")));
    }

    @Test
    void closedStateConsistencyCheckRejectsMissingClosingFields() {
        Organization org = organizationService.create("Org", "cs-state", "UTC", "MXN", "es-MX");
        UUID branchId = branchService.create(org.getId(), "Centro", "centro", null).getId();
        UUID registerId = insertRegister(org.getId(), branchId, "register-1");
        UUID userId = insertUser(org.getId());
        Timestamp now = Timestamp.from(Instant.now());

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_SHIFT,
                        UUID.randomUUID(), org.getId(), branchId, registerId, userId, null, "CLOSED",
                        new BigDecimal("10.0000"), new BigDecimal("10.0000"), now, null, 1, now, now));
    }

    @Test
    void openStateConsistencyCheckRejectsClosingFields() {
        Organization org = organizationService.create("Org", "cs-state2", "UTC", "MXN", "es-MX");
        UUID branchId = branchService.create(org.getId(), "Centro", "centro", null).getId();
        UUID registerId = insertRegister(org.getId(), branchId, "register-1");
        UUID userId = insertUser(org.getId());
        Timestamp now = Timestamp.from(Instant.now());

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_SHIFT,
                        UUID.randomUUID(), org.getId(), branchId, registerId, userId, userId, "OPEN",
                        new BigDecimal("10.0000"), null, now, now, 0, now, now));
    }

    @Test
    void negativeOpeningAmountIsRejected() {
        Organization org = organizationService.create("Org", "cs-neg", "UTC", "MXN", "es-MX");
        UUID branchId = branchService.create(org.getId(), "Centro", "centro", null).getId();
        UUID registerId = insertRegister(org.getId(), branchId, "register-1");
        UUID userId = insertUser(org.getId());

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_SHIFT, openShiftRow(org.getId(), branchId, registerId, userId, "-0.0001")));
    }

    private UUID insertUser(UUID organizationId) {
        UUID userId = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(
                "INSERT INTO users (id, organization_id, email, password_hash, status, version, created_at, updated_at) "
                        + "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                userId,
                organizationId,
                "user-" + userId.toString().substring(0, 8) + "@example.com",
                "hash",
                "ACTIVE",
                0,
                now,
                now);
        return userId;
    }
}
