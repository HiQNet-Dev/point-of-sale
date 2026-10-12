package com.hiqnet.pos.inventory.modifier.api;

import jakarta.validation.Valid;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.util.List;

public record ReplaceModifierInventoryEffectsRequest(
        @NotNull @Min(0) Long expectedVersion,
        @NotNull List<@Valid ModifierInventoryEffectRequest> effects) {}
