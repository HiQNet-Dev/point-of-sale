package com.hiqnet.pos.cashshift;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftStatus;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.HttpStatus;

/**
 * Cash-shift domain invariants without Spring: lifecycle (open -> close, no
 * reopen) and money precision (>= 0, at most 4 decimals, normalized, no silent
 * rounding).
 */
class CashShiftDomainTest {

    private static final UUID ORG = UUID.randomUUID();
    private static final UUID BRANCH = UUID.randomUUID();
    private static final UUID REGISTER = UUID.randomUUID();
    private static final UUID ACTOR = UUID.randomUUID();

    private static CashShift open(String amount) {
        return CashShift.open(ORG, BRANCH, REGISTER, ACTOR, new BigDecimal(amount), Instant.now());
    }

    @Test
    void openCreatesAnOpenShiftOwnedByTheActor() {
        CashShift shift = open("100.0000");

        assertThat(shift.getStatus()).isEqualTo(CashShiftStatus.OPEN);
        assertThat(shift.isOpen()).isTrue();
        assertThat(shift.getOrganizationId()).isEqualTo(ORG);
        assertThat(shift.getBranchId()).isEqualTo(BRANCH);
        assertThat(shift.getCashRegisterId()).isEqualTo(REGISTER);
        assertThat(shift.getOpenedByUserId()).isEqualTo(ACTOR);
        assertThat(shift.getOpenedAt()).isNotNull();
        assertThat(shift.getClosedAt()).isNull();
        assertThat(shift.getClosedByUserId()).isNull();
        assertThat(shift.getClosingDeclaredAmount()).isNull();
        assertThat(shift.getVersion()).isZero();
    }

    @Test
    void openNormalizesAmountToFourDecimalsExactly() {
        assertThat(open("0").getOpeningAmount()).isEqualByComparingTo("0.0000");
        assertThat(open("0").getOpeningAmount().scale()).isEqualTo(4);
        assertThat(open("12.5").getOpeningAmount()).isEqualTo(new BigDecimal("12.5000"));
        assertThat(open("0.0001").getOpeningAmount()).isEqualTo(new BigDecimal("0.0001"));
    }

    @Test
    void openRejectsNegativeAmount() {
        assertThatThrownBy(() -> open("-0.0001"))
                .isInstanceOf(BusinessException.class)
                .satisfies(e -> assertThat(((BusinessException) e).getStatus()).isEqualTo(HttpStatus.BAD_REQUEST))
                .satisfies(e -> assertThat(((BusinessException) e).getCode()).isEqualTo(ErrorCodes.VALIDATION_ERROR));
    }

    @Test
    void openRejectsAmountWithMoreThanFourDecimalsWithoutRounding() {
        assertThatThrownBy(() -> open("10.12345"))
                .isInstanceOf(BusinessException.class)
                .satisfies(e -> assertThat(((BusinessException) e).getCode()).isEqualTo(ErrorCodes.VALIDATION_ERROR));
    }

    @Test
    void openRejectsAmountBeyondColumnPrecision() {
        // 16 integer digits exceed NUMERIC(19,4) (max 15 integer digits).
        assertThatThrownBy(() -> open("1000000000000000"))
                .isInstanceOf(BusinessException.class)
                .satisfies(e -> assertThat(((BusinessException) e).getCode()).isEqualTo(ErrorCodes.VALIDATION_ERROR));
    }

    @Test
    void openAcceptsAmountAtColumnPrecisionBoundary() {
        assertThat(open("999999999999999.9999").getOpeningAmount())
                .isEqualByComparingTo("999999999999999.9999");
    }

    @Test
    void openRejectsNullRequiredReferences() {
        assertThatThrownBy(() -> CashShift.open(null, BRANCH, REGISTER, ACTOR, BigDecimal.ONE, Instant.now()))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> CashShift.open(ORG, null, REGISTER, ACTOR, BigDecimal.ONE, Instant.now()))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> CashShift.open(ORG, BRANCH, null, ACTOR, BigDecimal.ONE, Instant.now()))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> CashShift.open(ORG, BRANCH, REGISTER, null, BigDecimal.ONE, Instant.now()))
                .isInstanceOf(BusinessException.class);
    }

    @Test
    void closeTransitionsToClosedAndStoresTheCloser() {
        CashShift shift = open("100.0000");
        UUID closer = UUID.randomUUID();
        Instant closedAt = Instant.now();

        shift.close(new BigDecimal("250.5000"), closer, closedAt);

        assertThat(shift.getStatus()).isEqualTo(CashShiftStatus.CLOSED);
        assertThat(shift.isOpen()).isFalse();
        assertThat(shift.getClosingDeclaredAmount()).isEqualTo(new BigDecimal("250.5000"));
        assertThat(shift.getClosedByUserId()).isEqualTo(closer);
        assertThat(shift.getClosedAt()).isEqualTo(closedAt);
    }

    @Test
    void closeRejectsAlreadyClosedShift() {
        CashShift shift = open("100.0000");
        shift.close(new BigDecimal("100.0000"), ACTOR, Instant.now());

        assertThatThrownBy(() -> shift.close(new BigDecimal("100.0000"), ACTOR, Instant.now()))
                .isInstanceOf(BusinessException.class)
                .satisfies(e -> assertThat(((BusinessException) e).getStatus()).isEqualTo(HttpStatus.CONFLICT))
                .satisfies(e -> assertThat(((BusinessException) e).getCode())
                        .isEqualTo(ErrorCodes.CASH_SHIFT_ALREADY_CLOSED));
    }

    @Test
    void closeRejectsNegativeDeclaredAmount() {
        CashShift shift = open("100.0000");

        assertThatThrownBy(() -> shift.close(new BigDecimal("-1.0000"), ACTOR, Instant.now()))
                .isInstanceOf(BusinessException.class);
        assertThat(shift.isOpen()).isTrue();
    }

    @Test
    void closeRejectsDeclaredAmountWithMoreThanFourDecimals() {
        CashShift shift = open("100.0000");

        assertThatThrownBy(() -> shift.close(new BigDecimal("1.00001"), ACTOR, Instant.now()))
                .isInstanceOf(BusinessException.class);
        assertThat(shift.isOpen()).isTrue();
    }
}
