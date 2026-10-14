package com.hiqnet.pos.inventory.stock.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.stock.application.InventoryStockService;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRow;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.security.SecurityRequirement;
import io.swagger.v3.oas.annotations.tags.Tag;
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
@Tag(name = "Branch Stock", description = "Existencias y movimientos de inventario por sucursal.")
public class InventoryStockController {

    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort STOCK_ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));
    private static final Sort MOVEMENT_ORDER = Sort.by(Sort.Order.desc("occurredAt"), Sort.Order.desc("id"));

    private final InventoryStockService inventoryStockService;

    public InventoryStockController(InventoryStockService inventoryStockService) {
        this.inventoryStockService = inventoryStockService;
    }

    @GetMapping
    @Operation(
            summary = "Lista las existencias de una sucursal",
            description = "Existencias de inventario de la sucursal, con filtros opcionales "
                    + "`q` (code/name), `status` y `baseUnit`. Permission requerida: `inventory.stock.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Página de existencias"),
        @ApiResponse(responseCode = "400", description = "Parámetros de paginación inválidos (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `inventory.stock.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Sucursal inexistente o de otra organización (BRANCH_NOT_FOUND)", content = @Content)
    })
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
    @Operation(
            summary = "Obtiene la existencia de un insumo en una sucursal",
            description = "Balance de inventario del insumo solicitado en la sucursal. "
                    + "Permission requerida: `inventory.stock.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Existencia del insumo"),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `inventory.stock.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Sucursal o insumo inexistente/ajeno (BRANCH_NOT_FOUND / INVENTORY_ITEM_NOT_FOUND)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'inventory.stock.read')")
    public InventoryStockResponse get(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID inventoryItemId) {
        return inventoryStockService.get(organizationId, branchId, inventoryItemId);
    }

    @GetMapping("/{inventoryItemId}/movements")
    @Operation(
            summary = "Historial de movimientos de un insumo en una sucursal",
            description = "Página de movimientos de inventario (apertura y ajustes), ordenados de más "
                    + "reciente a más antiguo. Permission requerida: `inventory.stock.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Página de movimientos"),
        @ApiResponse(responseCode = "400", description = "Parámetros de paginación inválidos (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `inventory.stock.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Sucursal o insumo inexistente/ajeno", content = @Content)
    })
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
    @Operation(
            summary = "Registra la existencia de apertura de un insumo",
            description = "Fija la existencia inicial del insumo en la sucursal. `quantity` es un string "
                    + "decimal (hasta 6 decimales). Requiere `expectedVersion` para concurrencia optimista. "
                    + "Permission requerida: `inventory.stock.adjust`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Movimiento de apertura aplicado"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / INVALID_INVENTORY_QUANTITY (quantity no es string decimal)", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `inventory.stock.adjust` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Sucursal o insumo inexistente/ajeno", content = @Content),
        @ApiResponse(responseCode = "409", description = "Conflicto de estado (INVENTORY_OPENING_ALREADY_EXISTS / INVENTORY_ITEM_INACTIVE) o de versión (CONCURRENT_MODIFICATION)", content = @Content)
    })
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
    @Operation(
            summary = "Aplica un ajuste de inventario",
            description = "Ajusta la existencia del insumo por un delta con signo. `quantityDelta` es un "
                    + "string decimal (hasta 6 decimales, admite negativos según la política de la "
                    + "organización). Requiere `expectedVersion`. Permission requerida: `inventory.stock.adjust`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Ajuste aplicado"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / INVALID_INVENTORY_QUANTITY (quantityDelta no es string decimal)", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `inventory.stock.adjust` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Sucursal o insumo inexistente/ajeno", content = @Content),
        @ApiResponse(responseCode = "409", description = "Conflicto de versión (CONCURRENT_MODIFICATION) o de estado (INVENTORY_ITEM_INACTIVE / INVENTORY_HISTORY_ALREADY_EXISTS)", content = @Content)
    })
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
