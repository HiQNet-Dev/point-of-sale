package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.sales.application.SaleService;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.jdbc.core.JdbcTemplate;

/** Sales commands and their audit records commit atomically. */
@Import(SaleAuditRollbackTest.FailingAuditConfig.class)
class SaleAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private CashRegisterRepository cashRegisterRepository;

    @Autowired
    private CashShiftRepository cashShiftRepository;

    @Autowired
    private SaleService saleService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void auditFailureRollsBackSaleCreation() {
        RbacFixture fixture = fixture();
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(Branch.create(organization, "Branch", "audit-sales", null));
        CashRegister register = cashRegisterRepository.saveAndFlush(CashRegister.create(organization, branch, "register", "Register"));
        CashShift shift = cashShiftRepository.saveAndFlush(
                CashShift.open(fixture.orgId(), branch.getId(), register.getId(), fixture.userId(), BigDecimal.ZERO, Instant.now()));

        assertThrows(AuditException.class, () -> saleService.create(
                fixture.orgId(), branch.getId(), register.getId(), shift.getId(), fixture.userId()));

        Long sales = jdbcTemplate.queryForObject(
                "select count(*) from sales where cash_shift_id = ?", Long.class, shift.getId());
        assertThat(sales).isZero();
    }

    @TestConfiguration
    static class FailingAuditConfig {
        @Bean
        @Primary
        AuditPort failingAuditPort() {
            return (AuditRecord record) -> {
                throw new AuditException("audit unavailable (forced test failure)");
            };
        }
    }
}
