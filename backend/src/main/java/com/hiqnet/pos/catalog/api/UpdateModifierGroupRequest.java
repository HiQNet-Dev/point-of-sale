package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

/**
 * Partial update: only fields present are applied; {@code code} is immutable and
 * is not accepted here. {@code expectedVersion} is required.
 */
public record UpdateModifierGroupRequest(
        @NotNull @Min(0) Long expectedVersion,
        @Size(max = 120) String name,
        @Min(0) Integer minSelections,
        @Min(1) Integer maxSelections,
        ModifierGroupStatus status) {
}
