package com.hiqnet.pos.cashshift.application;

import java.util.UUID;

/**
 * Sales-facing lifecycle guard owned by the cash-shift application boundary.
 * Its implementation lives in Sales so CashShift never depends on a concrete
 * Sales repository.
 */
public interface OpenSalesForCashShiftPort {
    boolean hasOpenSales(UUID cashShiftId);
}
