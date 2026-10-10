package com.hiqnet.pos.catalog.api;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.util.List;
import java.util.UUID;

/**
 * Full replacement of a product's modifier-group configuration. The position in
 * {@code modifierGroupIds} IS the resulting {@code display_order}; an empty list
 * clears the configuration. Duplicate ids are rejected.
 */
public record ReplaceProductModifierGroupsRequest(
        @NotNull @Min(0) Long expectedVersion,
        @NotNull List<UUID> modifierGroupIds) {
}
