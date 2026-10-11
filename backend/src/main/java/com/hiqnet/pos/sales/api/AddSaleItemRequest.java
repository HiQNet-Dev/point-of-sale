package com.hiqnet.pos.sales.api;

import jakarta.validation.constraints.Max;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.util.List;
import java.util.UUID;

/** Client supplies identities and quantity only; all price fields are server-owned. */
public record AddSaleItemRequest(
        @NotNull @Min(0) Long expectedSaleVersion,
        @NotNull UUID productId,
        @NotNull @Min(1) @Max(1_000_000) Integer quantity,
        List<UUID> modifierOptionIds) {}
