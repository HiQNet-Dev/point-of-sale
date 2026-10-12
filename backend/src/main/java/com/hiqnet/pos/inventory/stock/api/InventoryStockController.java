package com.hiqnet.pos.inventory.stock.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.stock.application.InventoryStockService;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRow;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.validation.Valid;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import tools.jackson.databind.JsonNode;

@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches/{branchId}/inventory-items")
public class InventoryStockController {

    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort STOCK_ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));
    private static final Sort MOVEMENT_ORDER = Sort.by(Sort.Order.desc("occurredAt"), Sort.Order.desc("id"));

    private final InventoryStockService inventoryStockService;

    public InventoryStockController(InventoryStockService inventoryStockService) {
        this.inventoryStockService = inventoryStockService;
    }

    @GetMapping
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'inventory.stock.read')")
    public PageResponse<InventoryStockResponse> list(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) String q,
            @RequestParam(required = false) InventoryItemStatus status,
            @RequestParam(required = false) InventoryBaseUnit baseUnit) {
        validatePaging(page, size);
        return PageResponse.from(
                inventoryStockService.list(
                        organizationId, branchId, q, status, baseUnit, PageRequest.of(page, size, STOCK_ORDER)),
                value -> value);
    }

    @GetMapping("/{inventoryItemId}")
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'inventory.stock.read')")
    public InventoryStockResponse get(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID inventoryItemId) {
        return inventoryStockService.get(organizationId, branchId, inventoryItemId);
    }

    @GetMapping("/{inventoryItemId}/movements")
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'inventory.stock.read')")
    public PageResponse<InventoryMovementResponse> movements(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID inventoryItemId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {
        validatePaging(page, size);
        Page<InventoryMovementRow> rows = inventoryStockService.history(
                organizationId, branchId, inventoryItemId, PageRequest.of(page, size, MOVEMENT_ORDER));
        return PageResponse.from(rows, InventoryMovementResponse::from);
    }

    @PostMapping("/{inventoryItemId}/opening-stock")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'inventory.stock.adjust')")
    public InventoryStockMutationResponse opening(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID inventoryItemId,
            @AuthenticationPrincipal Jwt jwt,
            @Valid @RequestBody OpenInventoryStockRequest request) {
        return inventoryStockService.open(
                organizationId,
                branchId,
                inventoryItemId,
                actorId(jwt),
                request.expectedVersion(),
                text(request.quantity(), "quantity"),
                optionalText(request.reason(), "reason"));
    }

    @PostMapping("/{inventoryItemId}/adjustments")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'inventory.stock.adjust')")
    public InventoryStockMutationResponse adjust(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID inventoryItemId,
            @AuthenticationPrincipal Jwt jwt,
            @Valid @RequestBody AdjustInventoryStockRequest request) {
        return inventoryStockService.adjust(
                organizationId,
                branchId,
                inventoryItemId,
                actorId(jwt),
                request.expectedVersion(),
                text(request.quantityDelta(), "quantityDelta"),
                text(request.reason(), "reason"));
    }

    private static String text(JsonNode node, String field) {
        if (node == null || !node.isString()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_INVENTORY_QUANTITY, field + " must be a JSON string");
        }
        return node.stringValue();
    }

    private static String optionalText(JsonNode node, String field) {
        if (node == null || node.isNull()) {
            return null;
        }
        return text(node, field);
    }

    private static UUID actorId(Jwt jwt) {
        try {
            return UUID.fromString(jwt.getSubject());
        } catch (Exception exception) {
            throw new BusinessException(
                    HttpStatus.UNAUTHORIZED, ErrorCodes.AUTHENTICATION_REQUIRED, "Authenticated user is required");
        }
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }
}
