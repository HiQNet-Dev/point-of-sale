package com.hiqnet.pos.inventory.recipe.api;

import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus;
import com.hiqnet.pos.inventory.recipe.domain.ProductInventoryRecipe;
import java.time.Instant;
import java.util.List;
import java.util.UUID;

public record ProductInventoryRecipeResponse(UUID id, UUID organizationId, UUID productId,
        InventoryRecipeStatus status, long version, Instant createdAt, Instant updatedAt,
        List<InventoryRecipeComponentResponse> components) {
    public static ProductInventoryRecipeResponse from(ProductInventoryRecipe recipe,
            List<InventoryRecipeComponentResponse> components) {
        return new ProductInventoryRecipeResponse(recipe.getId(), recipe.getOrganizationId(), recipe.getProductId(),
                recipe.getStatus(), recipe.getVersion(), recipe.getCreatedAt(), recipe.getUpdatedAt(), List.copyOf(components));
    }
}
