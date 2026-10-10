package com.hiqnet.pos.cashregister.application;

import com.hiqnet.pos.cashregister.domain.CashRegisterStatus;

/**
 * Partial update payload for a cash register. Null fields are left unchanged;
 * {@code code} is immutable and is not represented here, and the register's
 * organization/branch cannot be changed.
 */
public record UpdateCashRegisterCommand(
        long expectedVersion,
        String name,
        CashRegisterStatus status) {
}
