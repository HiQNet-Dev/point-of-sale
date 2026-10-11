package com.hiqnet.pos.payment.api;

import com.hiqnet.pos.payment.domain.Payment;
import com.hiqnet.pos.payment.domain.PaymentMethod;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

/** Historical, non-sensitive representation of an immutable checkout payment. */
public record PaymentResponse(UUID id, PaymentMethod method, BigDecimal amount, BigDecimal tenderedAmount,
        BigDecimal changeAmount, int position, Instant createdAt) {
    public static PaymentResponse from(Payment payment) {
        return new PaymentResponse(payment.getId(), payment.getMethod(), payment.getAmount(), payment.getTenderedAmount(),
                payment.getChangeAmount(), payment.getPosition(), payment.getCreatedAt());
    }
}
