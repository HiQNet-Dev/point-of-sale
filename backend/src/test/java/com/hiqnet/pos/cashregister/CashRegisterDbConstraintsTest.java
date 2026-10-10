package com.hiqnet.pos.cashregister;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
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
 * The database is the final authority on cash-register integrity: the composite
 * FK keeps a register in the SAME organization as its branch, the unique index
 * keeps codes unique per branch, and the CHECK constraints enforce the slug
 * format and a non-blank name.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
class CashRegisterDbConstraintsTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private BranchService branchService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private static final String INSERT_REGISTER =
            "INSERT INTO cash_registers (id, organization_id, branch_id, code, name, status, version, "
                    + "created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static Object[] registerRow(UUID orgId, UUID branchId, String code, String name) {
        Timestamp now = Timestamp.from(Instant.now());
        return new Object[] {UUID.randomUUID(), orgId, branchId, code, name, "ACTIVE", 0, now, now};
    }

    private UUID insertRegister(UUID orgId, UUID branchId, String code, String name) {
        Object[] row = registerRow(orgId, branchId, code, name);
        jdbcTemplate.update(INSERT_REGISTER, row);
        return (UUID) row[0];
    }

    @Test
    void registerCannotReferenceBranchOfAnotherOrganization() {
        Organization orgA = organizationService.create("Org A", "cr-fk-a", "UTC", "MXN", "es-MX");
        Organization orgB = organizationService.create("Org B", "cr-fk-b", "UTC", "MXN", "es-MX");
        UUID branchA = branchService.create(orgA.getId(), "Centro", "centro", null).getId();

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_REGISTER, registerRow(orgB.getId(), branchA, "register-1", "Front")));
    }

    @Test
    void registerCodeIsUniquePerBranch() {
        Organization org = organizationService.create("Org", "cr-code", "UTC", "MXN", "es-MX");
        UUID branchA = branchService.create(org.getId(), "Centro", "centro", null).getId();
        UUID branchB = branchService.create(org.getId(), "Norte", "norte", null).getId();
        insertRegister(org.getId(), branchA, "register-1", "Front");

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_REGISTER, registerRow(org.getId(), branchA, "register-1", "Other")));

        assertDoesNotThrow(() -> jdbcTemplate.update(
                INSERT_REGISTER, registerRow(org.getId(), branchB, "register-1", "Front")));
    }

    @Test
    void registerCodeFormatCheckRejectsNonCanonicalCode() {
        Organization org = organizationService.create("Org", "cr-slug", "UTC", "MXN", "es-MX");
        Branch branch = branchService.create(org.getId(), "Centro", "centro", null);

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_REGISTER, registerRow(org.getId(), branch.getId(), "Register 1", "Front")));
    }

    @Test
    void registerNameCheckRejectsBlankName() {
        Organization org = organizationService.create("Org", "cr-name", "UTC", "MXN", "es-MX");
        Branch branch = branchService.create(org.getId(), "Centro", "centro", null);

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_REGISTER, registerRow(org.getId(), branch.getId(), "register-1", "   ")));
    }

    @Test
    void registerStatusCheckRejectsUnknownStatus() {
        Organization org = organizationService.create("Org", "cr-status", "UTC", "MXN", "es-MX");
        Branch branch = branchService.create(org.getId(), "Centro", "centro", null);
        Timestamp now = Timestamp.from(Instant.now());

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_REGISTER,
                        UUID.randomUUID(),
                        org.getId(),
                        branch.getId(),
                        "register-1",
                        "Front",
                        "BROKEN",
                        0,
                        now,
                        now));
    }
}
