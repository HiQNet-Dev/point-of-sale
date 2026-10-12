package com.hiqnet.pos.inventory.api;

import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.application.UpdateInventoryItemCommand;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import jakarta.validation.Valid;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/inventory-items")
public class InventoryItemController {

    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final InventoryItemService inventoryItemService;

    public InventoryItemController(InventoryItemService inventoryItemService) {
        this.inventoryItemService = inventoryItemService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create an inventory item inside an organization")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR, INVALID_CODE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (INVENTORY_ITEM_CODE_ALREADY_EXISTS)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.item.write)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.item.write')")
    public InventoryItemResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateInventoryItemRequest request) {
        return InventoryItemResponse.from(inventoryItemService.create(
                organizationId,
                request.code(),
                request.name(),
                request.description(),
                request.baseUnit(),
                request.fractionalUnitAllowed()));
    }

    @GetMapping
    @Operation(summary = "List inventory items of an organization (paginated; filters q, status and baseUnit)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.item.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.item.read')")
    public PageResponse<InventoryItemResponse> list(
            @PathVariable UUID organizationId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) String q,
            @RequestParam(required = false) InventoryItemStatus status,
            @RequestParam(required = false) InventoryBaseUnit baseUnit) {
        validatePaging(page, size);
        Page<InventoryItem> items = inventoryItemService.list(
                organizationId, q, status, baseUnit, PageRequest.of(page, size, ORDER));
        return PageResponse.from(items, InventoryItemResponse::from);
    }

    @GetMapping("/{inventoryItemId}")
    @Operation(summary = "Get an inventory item by id, scoped to its organization")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or item not found (ORGANIZATION_NOT_FOUND, INVENTORY_ITEM_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.item.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.item.read')")
    public InventoryItemResponse get(
            @PathVariable UUID organizationId, @PathVariable UUID inventoryItemId) {
        return InventoryItemResponse.from(inventoryItemService.get(organizationId, inventoryItemId));
    }

    @PatchMapping("/{inventoryItemId}")
    @Operation(summary = "Partially update an inventory item; its code is immutable")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or item not found (ORGANIZATION_NOT_FOUND, INVENTORY_ITEM_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (inventory.item.write)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'inventory.item.write')")
    public InventoryItemResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID inventoryItemId,
            @Valid @RequestBody UpdateInventoryItemRequest request) {
        return InventoryItemResponse.from(inventoryItemService.update(
                organizationId,
                inventoryItemId,
                new UpdateInventoryItemCommand(
                        request.expectedVersion(),
                        request.name(),
                        request.description(),
                        request.baseUnit(),
                        request.fractionalUnitAllowed(),
                        request.status())));
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR,
                    "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }
}
