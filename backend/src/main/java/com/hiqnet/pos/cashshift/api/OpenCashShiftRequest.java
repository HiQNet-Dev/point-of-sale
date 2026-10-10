package com.hiqnet.pos.cashshift.api;

import jakarta.validation.constraints.NotNull;
import java.math.BigDecimal;

/**
 * Open-shift payload. Only the declared opening amount is client-supplied; the
 * organization, branch, register and the opening actor come from the path and
 * the authenticated principal (never the body).
 */
public record OpenCashShiftRequest(@NotNull BigDecimal openingAmount) {
}
