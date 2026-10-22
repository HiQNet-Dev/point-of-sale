package com.hiqnet.pos.inventory.modifier.api;

import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectChange;
import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectsService;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;
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
@RequestMapping("/api/v1/organizations/{organizationId}/modifier-options/{modifierOptionId}/inventory-effects")
@Tag(name = "Inventory Effects", description = "Efectos de las opciones de modificador sobre el inventario.")
public class ModifierOptionInventoryEffectsController {
    private final ModifierInventoryEffectsService effectsService;
    public ModifierOptionInventoryEffectsController(ModifierInventoryEffectsService effectsService) {
        this.effectsService = effectsService;
    }

    @GetMapping
    @Operation(summary = "Get Inventory effects configured for a modifier option")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Configuration found", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, modifier option, or configuration not found", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.recipe.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.recipe.read')")
    public ModifierInventoryEffectsResponse get(@PathVariable UUID organizationId, @PathVariable UUID modifierOptionId) {
        return effectsService.get(organizationId, modifierOptionId);
    }

    @PutMapping
    @Operation(summary = "Create or replace Inventory effects for a modifier option")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Effects replaced", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid or duplicate effects", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, modifier option, or Inventory item not found", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification or inactive Inventory item", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.recipe.write)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.recipe.write')")
    public ModifierInventoryEffectsResponse replace(@PathVariable UUID organizationId,
            @PathVariable UUID modifierOptionId, @Valid @RequestBody ReplaceModifierInventoryEffectsRequest request) {
        var effects = request.effects().stream()
                .map(effect -> new ModifierInventoryEffectChange(effect.inventoryItemId(),
                        InventoryQuantity.parse(effect.quantityDelta(), "quantityDelta")))
                .toList();
        return effectsService.replace(organizationId, modifierOptionId, request.expectedVersion(), effects);
    }
}
