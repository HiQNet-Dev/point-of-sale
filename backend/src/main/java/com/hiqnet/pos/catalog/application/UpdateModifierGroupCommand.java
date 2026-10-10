package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;

/**
 * Partial update payload for a modifier group. Null fields are left unchanged;
 * {@code code} is immutable and is not represented here. {@code minSelections}
 * and {@code maxSelections} are applied together when either is present.
 */
public record UpdateModifierGroupCommand(
        long expectedVersion,
        String name,
        Integer minSelections,
        Integer maxSelections,
        ModifierGroupStatus status) {
}
