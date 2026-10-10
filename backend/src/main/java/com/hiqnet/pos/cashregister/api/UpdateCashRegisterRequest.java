package com.hiqnet.pos.cashregister.api;

import com.hiqnet.pos.cashregister.domain.CashRegisterStatus;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

/**
 * Partial update: only fields present are applied; {@code code} is immutable and
 * the register cannot be moved to another organization/branch.
 * {@code expectedVersion} is required.
 */
public record UpdateCashRegisterRequest(
        @NotNull @Min(0) Long expectedVersion,
        @Size(max = 120) String name,
        CashRegisterStatus status) {
}
