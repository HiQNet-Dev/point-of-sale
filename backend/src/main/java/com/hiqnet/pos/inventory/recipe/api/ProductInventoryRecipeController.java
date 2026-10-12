package com.hiqnet.pos.inventory.recipe.api;

import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeComponentChange;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeService;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import jakarta.validation.Valid;
import java.util.UUID;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/products/{productId}/inventory-recipe")
public class ProductInventoryRecipeController {
    private final InventoryRecipeService recipeService;
    public ProductInventoryRecipeController(InventoryRecipeService recipeService) { this.recipeService = recipeService; }

    @GetMapping
    @Operation(summary = "Get the configured Inventory recipe for a product")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Recipe found", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, Product, or recipe not found", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.recipe.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.recipe.read')")
    public ProductInventoryRecipeResponse get(@PathVariable UUID organizationId, @PathVariable UUID productId) {
        return recipeService.get(organizationId, productId);
    }

    @PutMapping
    @Operation(summary = "Create or replace a product Inventory recipe")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Recipe replaced", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid components, quantities, or active-empty recipe", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, Product, or Inventory item not found", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification or inactive Inventory item", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.recipe.write)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.recipe.write')")
    public ProductInventoryRecipeResponse replace(@PathVariable UUID organizationId, @PathVariable UUID productId,
            @Valid @RequestBody ReplaceProductInventoryRecipeRequest request) {
        var components = request.components().stream()
                .map(component -> new InventoryRecipeComponentChange(component.inventoryItemId(),
                        InventoryQuantity.parse(component.quantity(), "quantity")))
                .toList();
        return recipeService.replace(organizationId, productId, request.expectedVersion(), request.status(), components);
    }
}
