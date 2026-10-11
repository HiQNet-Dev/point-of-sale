package com.hiqnet.pos.sales;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.organization.domain.Organization;
import java.math.BigDecimal;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;

/** V14 payment checks and composite foreign keys are enforced by PostgreSQL. */
class PaymentDbConstraintsTest extends AbstractRbacIntegrationTest {

    private static final String INSERT_SALE = "INSERT INTO sales (id, organization_id, branch_id, cash_register_id, cash_shift_id, "
            + "created_by_user_id, status, total_amount, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'OPEN', 0, 0, ?, ?)";
    private static final String INSERT_PAYMENT = "INSERT INTO payments (id, organization_id, branch_id, cash_register_id, cash_shift_id, "
            + "sale_id, position, method, amount, tendered_amount, change_amount, created_by_user_id, created_at) "
            + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    @Autowired private BranchRepository branchRepository;
    @Autowired private CashRegisterRepository cashRegisterRepository;
    @Autowired private CashShiftRepository cashShiftRepository;
    @Autowired private JdbcTemplate jdbcTemplate;

    @Test
    void cashAndCardMethodChecksRejectInvalidCombinations() {
        assertDoesNotThrow(() -> insertPayment(scope(), 1, "CASH", "10.0000", "15.0000", "5.0000", null));
        assertDoesNotThrow(() -> insertPayment(scope(), 1, "CARD", "10.0000", null, "0.0000", null));
        assertRejected(() -> insertPayment(scope(), 1, "CASH", "10.0000", null, "0.0000", null));
        assertRejected(() -> insertPayment(scope(), 1, "CASH", "10.0000", "9.0000", "0.0000", null));
        assertRejected(() -> insertPayment(scope(), 1, "CASH", "10.0000", "15.0000", "4.0000", null));
        assertRejected(() -> insertPayment(scope(), 1, "CARD", "10.0000", "10.0000", "0.0000", null));
        assertRejected(() -> insertPayment(scope(), 1, "CARD", "10.0000", null, "0.0001", null));
        assertRejected(() -> insertPayment(scope(), 1, "TRANSFER", "10.0000", null, "0.0000", null));
        assertRejected(() -> insertPayment(scope(), 1, "CARD", "0.0000", null, "0.0000", null));
        assertRejected(() -> insertPayment(scope(), 1, "CARD", "-0.0001", null, "0.0000", null));
    }

    @Test
    void positionMustBePositiveAndUniqueWithinSale() {
        Scope scope = scope();
        assertRejected(() -> insertPayment(scope, 0, "CARD", "10.0000", null, "0.0000", null));
        insertPayment(scope, 1, "CARD", "10.0000", null, "0.0000", null);
        assertRejected(() -> insertPayment(scope, 1, "CARD", "10.0000", null, "0.0000", null));
    }

    @Test
    void paymentCannotCrossOrganizationOrUseAnotherOrganizationsActor() {
        Scope organizationA = scope();
        Scope organizationB = scope();
        assertRejected(() -> insertPayment(organizationB.organizationId(), organizationB.branchId(), organizationB.registerId(),
                organizationB.shiftId(), organizationA.saleId(), 1, "CARD", "10.0000", null, "0.0000", organizationB.userId()));
        assertRejected(() -> insertPayment(organizationA.organizationId(), organizationA.branchId(), organizationA.registerId(),
                organizationA.shiftId(), organizationA.saleId(), 1, "CARD", "10.0000", null, "0.0000", organizationB.userId()));
    }

    @Test
    void paymentCannotUseSaleWithWrongBranchRegisterOrShift() {
        Scope saleScope = scope();
        Scope otherScope = scope();
        assertRejected(() -> insertPayment(saleScope.organizationId(), otherScope.branchId(), saleScope.registerId(),
                saleScope.shiftId(), saleScope.saleId(), 1, "CARD", "10.0000", null, "0.0000", saleScope.userId()));
        assertRejected(() -> insertPayment(saleScope.organizationId(), saleScope.branchId(), otherScope.registerId(),
                saleScope.shiftId(), saleScope.saleId(), 1, "CARD", "10.0000", null, "0.0000", saleScope.userId()));
        assertRejected(() -> insertPayment(saleScope.organizationId(), saleScope.branchId(), saleScope.registerId(),
                otherScope.shiftId(), saleScope.saleId(), 1, "CARD", "10.0000", null, "0.0000", saleScope.userId()));
    }

    private Scope scope() {
        RbacFixture fixture = fixture();
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(Branch.create(organization, "Branch", "branch", null));
        CashRegister register = cashRegisterRepository.saveAndFlush(CashRegister.create(organization, branch, "register", "Register"));
        CashShift shift = cashShiftRepository.saveAndFlush(CashShift.open(
                organization.getId(), branch.getId(), register.getId(), fixture.userId(), BigDecimal.ZERO, Instant.now()));
        UUID saleId = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(INSERT_SALE, saleId, organization.getId(), branch.getId(), register.getId(), shift.getId(), fixture.userId(), now, now);
        return new Scope(organization.getId(), branch.getId(), register.getId(), shift.getId(), saleId, fixture.userId());
    }

    private void insertPayment(Scope scope, int position, String method, String amount, String tendered, String change, UUID actor) {
        insertPayment(scope.organizationId(), scope.branchId(), scope.registerId(), scope.shiftId(), scope.saleId(), position,
                method, amount, tendered, change, actor == null ? scope.userId() : actor);
    }

    private void insertPayment(UUID organizationId, UUID branchId, UUID registerId, UUID shiftId, UUID saleId, int position,
            String method, String amount, String tendered, String change, UUID actor) {
        jdbcTemplate.update(INSERT_PAYMENT, UUID.randomUUID(), organizationId, branchId, registerId, shiftId, saleId,
                position, method, new BigDecimal(amount), tendered == null ? null : new BigDecimal(tendered), new BigDecimal(change),
                actor, Timestamp.from(Instant.now()));
    }

    private static void assertRejected(org.junit.jupiter.api.function.Executable operation) {
        assertThrows(DataIntegrityViolationException.class, operation);
    }

    private record Scope(UUID organizationId, UUID branchId, UUID registerId, UUID shiftId, UUID saleId, UUID userId) {}
}
