package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.application.ProductModifierService;
import com.hiqnet.pos.catalog.application.ProductModifierAssignment;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import java.util.List;
import java.util.UUID;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Organization-scoped product <-> modifier-group configuration. Reading the
 * assignment list requires {@code product.read}; replacing it requires
 * {@code product.modifiers.manage}. The list is bounded configuration data and
 * is not paginated. The product's version is the concurrency boundary.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/products/{productId}/modifier-groups")
@Tag(name = "Modifiers", description = "Grupos y opciones de modificadores de producto.")
public class ProductModifierGroupController {

    private final ProductModifierService productModifierService;

    public ProductModifierGroupController(ProductModifierService productModifierService) {
        this.productModifierService = productModifierService;
    }

    @GetMapping
    @Operation(summary = "List the modifier groups attached to a product, ordered by displayOrder")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or product not found (ORGANIZATION_NOT_FOUND, PRODUCT_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (product.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'product.read')")
    public List<ProductModifierGroupResponse> list(
            @PathVariable UUID organizationId, @PathVariable UUID productId) {
        return productModifierService.list(organizationId, productId).stream()
                .map(ProductModifierGroupController::toResponse)
                .toList();
    }

    @PutMapping
    @Operation(summary = "Replace the modifier groups attached to a product (ordered; product version is the concurrency boundary)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Replaced", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation, e.g. duplicate ids (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, product or modifier group not found (ORGANIZATION_NOT_FOUND, PRODUCT_NOT_FOUND, MODIFIER_GROUP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (product.modifiers.manage)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'product.modifiers.manage')")
    public List<ProductModifierGroupResponse> replace(
            @PathVariable UUID organizationId,
            @PathVariable UUID productId,
            @Valid @RequestBody ReplaceProductModifierGroupsRequest request) {
        return productModifierService
                .replace(organizationId, productId, request.expectedVersion(), request.modifierGroupIds())
                .stream()
                .map(ProductModifierGroupController::toResponse)
                .toList();
    }

    private static ProductModifierGroupResponse toResponse(ProductModifierAssignment assignment) {
        return ProductModifierGroupResponse.from(assignment.group(), assignment.displayOrder());
    }
}
