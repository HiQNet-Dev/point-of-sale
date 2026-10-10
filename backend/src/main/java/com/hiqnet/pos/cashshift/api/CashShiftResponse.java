package com.hiqnet.pos.cashshift.api;

import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftStatus;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

public record CashShiftResponse(
        UUID id,
        UUID organizationId,
        UUID branchId,
        UUID cashRegisterId,
        UUID openedByUserId,
        UUID closedByUserId,
        CashShiftStatus status,
        BigDecimal openingAmount,
        BigDecimal closingDeclaredAmount,
        Instant openedAt,
        Instant closedAt,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static CashShiftResponse from(CashShift shift) {
        return new CashShiftResponse(
                shift.getId(),
                shift.getOrganizationId(),
                shift.getBranchId(),
                shift.getCashRegisterId(),
                shift.getOpenedByUserId(),
                shift.getClosedByUserId(),
                shift.getStatus(),
                shift.getOpeningAmount(),
                shift.getClosingDeclaredAmount(),
                shift.getOpenedAt(),
                shift.getClosedAt(),
                shift.getVersion(),
                shift.getCreatedAt(),
                shift.getUpdatedAt());
    }
}
