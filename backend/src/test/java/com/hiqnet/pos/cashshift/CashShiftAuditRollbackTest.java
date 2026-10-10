package com.hiqnet.pos.cashshift;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.math.BigDecimal;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * A required cash-shift audit record that cannot be persisted FAILS the whole
 * mutation ({@code Propagation.MANDATORY}): the shift row is rolled back too, so
 * "business commit + missing audit" is impossible for BOTH open and close.
 * After such a failure the shift must be completely untouched (open: no row, so
 * the one-open slot is free; close: the row is still fully OPEN), and a
 * subsequent normal mutation of the same shift must succeed.
 */
@Import(CashShiftAuditRollbackTest.FailingAuditConfig.class)
class CashShiftAuditRollbackTest extends AbstractRbacIntegrationTest {

    /** Toggled per test: when true the audit port throws, forcing the rollback. */
    static final AtomicBoolean FAIL_AUDIT = new AtomicBoolean(true);

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @BeforeEach
    void armAuditFailure() {
        FAIL_AUDIT.set(true);
    }

    @Test
    void auditFailureRollsBackCashShiftOpenAndFreesTheRegister() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        UUID branchId = branchRepository
                .saveAndFlush(Branch.create(organization, "Centro", "centro", null))
                .getId();
        String registerId = createRegister(fx, branchId);

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000}"))
                .andExpect(status().isInternalServerError());

        // Nothing committed: no shift row AND no audit event.
        assertThat(count("select count(*) from cash_shifts where organization_id = ?", fx.orgId()))
                .isZero();
        assertThat(auditCount(fx.orgId())).isZero();

        // The one-open slot is free: a normal open on the same register now succeeds.
        FAIL_AUDIT.set(false);
        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000}"))
                .andExpect(status().isCreated());
        assertThat(count(
                        "select count(*) from cash_shifts where cash_register_id = ? and status = 'OPEN'",
                        UUID.fromString(registerId)))
                .isEqualTo(1);
    }

    @Test
    void auditFailureRollsBackCashShiftCloseLeavingTheRowFullyOpen() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        UUID branchId = branchRepository
                .saveAndFlush(Branch.create(organization, "Centro", "centro", null))
                .getId();
        String registerId = createRegister(fx, branchId);
        String shiftId = insertOpenShift(fx, branchId, UUID.fromString(registerId));

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":100.0000}"))
                .andExpect(status().isInternalServerError());

        // The business row rolled back completely: still OPEN, version, no closing data.
        Map<String, Object> row = jdbcTemplate.queryForMap(
                "select status, version, closed_at, closed_by_user_id, closing_declared_amount "
                        + "from cash_shifts where id = ?",
                UUID.fromString(shiftId));
        assertThat(row.get("status")).isEqualTo("OPEN");
        assertThat(((Number) row.get("version")).longValue()).isZero();
        assertThat(row.get("closed_at")).isNull();
        assertThat(row.get("closed_by_user_id")).isNull();
        assertThat(row.get("closing_declared_amount")).isNull();
        assertThat(auditCount(fx.orgId())).isZero();

        // A subsequent normal close of the same (still OPEN, version 0) row succeeds.
        FAIL_AUDIT.set(false);
        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":100.0000}"))
                .andExpect(status().isOk());
        assertThat(jdbcTemplate.queryForObject(
                        "select status from cash_shifts where id = ?", String.class, UUID.fromString(shiftId)))
                .isEqualTo("CLOSED");
    }

    private long count(String sql, Object... args) {
        Long value = jdbcTemplate.queryForObject(sql, Long.class, args);
        return value == null ? 0 : value;
    }

    private long auditCount(UUID organizationId) {
        return count("select count(*) from audit_events where organization_id = ?", organizationId);
    }

    private String createRegister(RbacFixture fx, UUID branchId) {
        UUID registerId = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(
                "INSERT INTO cash_registers (id, organization_id, branch_id, code, name, status, version, "
                        + "created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                registerId,
                fx.orgId(),
                branchId,
                "register-" + registerId.toString().substring(0, 8),
                "Front",
                "ACTIVE",
                0,
                now,
                now);
        return registerId.toString();
    }

    private String insertOpenShift(RbacFixture fx, UUID branchId, UUID registerId) {
        UUID shiftId = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(
                "INSERT INTO cash_shifts (id, organization_id, branch_id, cash_register_id, "
                        + "opened_by_user_id, closed_by_user_id, status, opening_amount, closing_declared_amount, "
                        + "opened_at, closed_at, version, created_at, updated_at) "
                        + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                shiftId,
                fx.orgId(),
                branchId,
                registerId,
                fx.userId(),
                null,
                "OPEN",
                new BigDecimal("100.0000"),
                null,
                now,
                null,
                0,
                now,
                now);
        return shiftId.toString();
    }

    private static String shiftsUrl(RbacFixture fx, UUID branchId, String registerId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers/" + registerId
                + "/cash-shifts";
    }

    @TestConfiguration
    static class FailingAuditConfig {
        @Bean
        @Primary
        AuditPort failingAuditPort() {
            return (AuditRecord record) -> {
                if (FAIL_AUDIT.get()) {
                    throw new AuditException("audit unavailable (forced test failure)");
                }
            };
        }
    }
}
