package com.hiqnet.pos.cashshift.api;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.math.BigDecimal;

/**
 * Close-shift payload. {@code expectedVersion} is required for optimistic
 * concurrency; the closing actor and the close timestamp are server-derived,
 * never client-supplied.
 */
public record CloseCashShiftRequest(
        @NotNull @Min(0) Long expectedVersion,
        @NotNull BigDecimal declaredCashAmount) {
}
