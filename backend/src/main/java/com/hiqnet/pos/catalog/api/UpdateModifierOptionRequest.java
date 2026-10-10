package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.ModifierOptionStatus;
import jakarta.validation.constraints.Digits;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;
import java.math.BigDecimal;

/**
 * Partial update: only fields present are applied; {@code code} is immutable and
 * is not accepted here. {@code expectedVersion} is required.
 */
public record UpdateModifierOptionRequest(
        @NotNull @Min(0) Long expectedVersion,
        @Size(max = 120) String name,
        @Digits(integer = 15, fraction = 4) BigDecimal priceDelta,
        @Min(0) Integer displayOrder,
        ModifierOptionStatus status) {
}
