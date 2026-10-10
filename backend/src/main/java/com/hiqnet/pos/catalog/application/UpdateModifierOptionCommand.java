package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.ModifierOptionStatus;
import java.math.BigDecimal;

/**
 * Partial update payload for a modifier option. Null fields are left unchanged;
 * {@code code} is immutable and is not represented here.
 */
public record UpdateModifierOptionCommand(
        long expectedVersion,
        String name,
        BigDecimal priceDelta,
        Integer displayOrder,
        ModifierOptionStatus status) {
}
