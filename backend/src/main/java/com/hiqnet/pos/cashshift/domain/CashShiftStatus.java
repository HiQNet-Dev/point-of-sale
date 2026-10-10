package com.hiqnet.pos.cashshift.domain;

/**
 * Lifecycle of a cash shift. Only two states exist: a shift is opened and later
 * closed exactly once. There is no reopen, cancellation, suspension or pause —
 * a CLOSED shift is historical and immutable.
 */
public enum CashShiftStatus {
    OPEN,
    CLOSED
}
