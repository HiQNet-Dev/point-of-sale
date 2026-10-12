package com.hiqnet.pos.inventory.recipe.api;

import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus;
import jakarta.validation.Valid;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.util.List;

public record ReplaceProductInventoryRecipeRequest(
        @NotNull @Min(0) Long expectedVersion,
        @NotNull InventoryRecipeStatus status,
        @NotNull @Valid List<@Valid InventoryRecipeComponentRequest> components) {}
