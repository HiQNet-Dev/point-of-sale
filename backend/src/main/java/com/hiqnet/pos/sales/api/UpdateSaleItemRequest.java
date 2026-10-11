package com.hiqnet.pos.sales.api;

import jakarta.validation.constraints.Max;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.util.List;
import java.util.UUID;

/** Product identity is deliberately immutable: replace a product via remove + add. */
public record UpdateSaleItemRequest(
        @NotNull @Min(0) Long expectedSaleVersion,
        @NotNull @Min(1) @Max(1_000_000) Integer quantity,
        List<UUID> modifierOptionIds) {}
