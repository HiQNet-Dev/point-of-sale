package com.hiqnet.pos.sales.api;

import com.hiqnet.pos.sales.domain.Sale;
import com.hiqnet.pos.sales.domain.SaleStatus;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

/** Summary response used for paginated sale lists. */
public record SaleResponse(UUID id, UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId,
        UUID createdByUserId, UUID cancelledByUserId, UUID completedByUserId, SaleStatus status, BigDecimal totalAmount, long version,
        Instant createdAt, Instant updatedAt, Instant cancelledAt, Instant completedAt) {
    public static SaleResponse from(Sale sale) {
        return new SaleResponse(sale.getId(), sale.getOrganizationId(), sale.getBranchId(), sale.getCashRegisterId(), sale.getCashShiftId(),
                sale.getCreatedByUserId(), sale.getCancelledByUserId(), sale.getCompletedByUserId(), sale.getStatus(), sale.getTotalAmount(), sale.getVersion(),
                sale.getCreatedAt(), sale.getUpdatedAt(), sale.getCancelledAt(), sale.getCompletedAt());
    }
}
