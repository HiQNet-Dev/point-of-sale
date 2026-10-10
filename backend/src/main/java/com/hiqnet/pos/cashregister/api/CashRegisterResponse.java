package com.hiqnet.pos.cashregister.api;

import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterStatus;
import java.time.Instant;
import java.util.UUID;

public record CashRegisterResponse(
        UUID id,
        UUID organizationId,
        UUID branchId,
        String code,
        String name,
        CashRegisterStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static CashRegisterResponse from(CashRegister register) {
        return new CashRegisterResponse(
                register.getId(),
                register.getOrganization().getId(),
                register.getBranch().getId(),
                register.getCode(),
                register.getName(),
                register.getStatus(),
                register.getVersion(),
                register.getCreatedAt(),
                register.getUpdatedAt());
    }
}
