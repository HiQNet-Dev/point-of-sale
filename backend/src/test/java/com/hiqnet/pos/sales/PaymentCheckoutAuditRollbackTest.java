package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.audit.infrastructure.AuditRecorder;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.payment.domain.PaymentMethod;
import com.hiqnet.pos.sales.api.CheckoutRequest;
import com.hiqnet.pos.sales.application.SaleService;
import com.hiqnet.pos.sales.domain.Sale;
import com.hiqnet.pos.sales.domain.SaleItem;
import com.hiqnet.pos.sales.domain.SaleItemRepository;
import com.hiqnet.pos.sales.domain.SaleRepository;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.jdbc.core.JdbcTemplate;

/** A late audit failure must roll back both Sale completion and every Payment row. */
@Import(PaymentCheckoutAuditRollbackTest.SelectiveAuditFailureConfig.class)
class PaymentCheckoutAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired private BranchRepository branchRepository;
    @Autowired private CashRegisterRepository cashRegisterRepository;
    @Autowired private CashShiftRepository cashShiftRepository;
    @Autowired private SaleRepository saleRepository;
    @Autowired private SaleItemRepository saleItemRepository;
    @Autowired private ProductRepository productRepository;
    @Autowired private SaleService saleService;
    @Autowired private JdbcTemplate jdbcTemplate;
    @Autowired private CheckoutAuditFailureControl auditFailureControl;

    @Test
    void auditFailureRollsBackAllCheckoutWritesAndANormalRetrySucceeds() {
        RbacFixture fixture = fixture();
        OperationalScope scope = operationalScope(fixture);
        Sale sale = saleRepository.saveAndFlush(Sale.open(
                fixture.orgId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(), fixture.userId()));
        Product product = productRepository.saveAndFlush(Product.create(scope.organization(), "SNAPSHOT", "Snapshot", new BigDecimal("10.0000")));
        saleItemRepository.saveAndFlush(SaleItem.create(fixture.orgId(), sale.getId(), product.getId(), "SNAPSHOT", "Snapshot",
                new BigDecimal("10.0000"), BigDecimal.ZERO, new BigDecimal("10.0000"), 1, 1));
        sale.recalculateTotal(new BigDecimal("10.0000"), Instant.now());
        saleRepository.saveAndFlush(sale);
        long auditsBefore = auditCount(fixture.orgId());

        auditFailureControl.failCheckoutAudit.set(true);
        assertThrows(AuditException.class, () -> saleService.checkout(fixture.orgId(), scope.branch().getId(), scope.register().getId(),
                scope.shift().getId(), sale.getId(), 1,
                List.of(new CheckoutRequest.PaymentInstruction(PaymentMethod.CARD, new BigDecimal("10.0000"), null)), fixture.userId()));

        assertThat(jdbcTemplate.queryForObject("select status from sales where id = ?", String.class, sale.getId())).isEqualTo("OPEN");
        assertThat(jdbcTemplate.queryForObject("select version from sales where id = ?", Long.class, sale.getId())).isEqualTo(1L);
        assertThat(jdbcTemplate.queryForObject("select completed_at from sales where id = ?", java.sql.Timestamp.class, sale.getId())).isNull();
        assertThat(jdbcTemplate.queryForObject("select completed_by_user_id from sales where id = ?", UUID.class, sale.getId())).isNull();
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class, sale.getId())).isZero();
        assertThat(auditCount(fixture.orgId())).isEqualTo(auditsBefore);

        auditFailureControl.failCheckoutAudit.set(false);
        saleService.checkout(fixture.orgId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(), sale.getId(), 1,
                List.of(new CheckoutRequest.PaymentInstruction(PaymentMethod.CARD, new BigDecimal("10.0000"), null)), fixture.userId());
        assertThat(jdbcTemplate.queryForObject("select status from sales where id = ?", String.class, sale.getId())).isEqualTo("COMPLETED");
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class, sale.getId())).isEqualTo(1L);
        assertThat(auditCountByAction(fixture.orgId(), "sale.completed")).isEqualTo(1L);
    }

    private OperationalScope operationalScope(RbacFixture fixture) {
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(Branch.create(organization, "Branch", "checkout-audit", null));
        CashRegister register = cashRegisterRepository.saveAndFlush(CashRegister.create(organization, branch, "register", "Register"));
        CashShift shift = cashShiftRepository.saveAndFlush(CashShift.open(
                fixture.orgId(), branch.getId(), register.getId(), fixture.userId(), BigDecimal.ZERO, Instant.now()));
        return new OperationalScope(organization, branch, register, shift);
    }

    private long auditCount(UUID organizationId) {
        return jdbcTemplate.queryForObject("select count(*) from audit_events where organization_id = ?", Long.class, organizationId);
    }

    private long auditCountByAction(UUID organizationId, String action) {
        return jdbcTemplate.queryForObject("select count(*) from audit_events where organization_id = ? and action = ?", Long.class, organizationId, action);
    }

    private record OperationalScope(Organization organization, Branch branch, CashRegister register, CashShift shift) {}

    static final class CheckoutAuditFailureControl {
        private final AtomicBoolean failCheckoutAudit = new AtomicBoolean();
    }

    @TestConfiguration
    static class SelectiveAuditFailureConfig {
        @Bean CheckoutAuditFailureControl checkoutAuditFailureControl() { return new CheckoutAuditFailureControl(); }

        @Bean @Primary
        AuditPort selectiveAuditPort(CheckoutAuditFailureControl control, AuditRecorder delegate) {
            return record -> {
                if (control.failCheckoutAudit.get() && "sale.completed".equals(record.action())) {
                    throw new AuditException("audit unavailable (forced checkout test failure)");
                }
                delegate.record(record);
            };
        }
    }
}
