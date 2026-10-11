package com.hiqnet.pos.sales.api;

import com.hiqnet.pos.payment.domain.PaymentMethod;
import jakarta.validation.Valid;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.math.BigDecimal;
import java.util.List;

/** Complete an OPEN sale atomically; prices, change, actors and positions stay server-owned. */
public record CheckoutRequest(@NotNull @Min(0) Long expectedSaleVersion, @NotNull List<@Valid PaymentInstruction> payments) {
    public record PaymentInstruction(@NotNull PaymentMethod method, @NotNull BigDecimal amount, BigDecimal tenderedAmount) {}
}
