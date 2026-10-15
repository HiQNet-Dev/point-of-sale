package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.application.CreatePurchaseOrderLineCommand;
import com.hiqnet.pos.purchasing.application.PurchaseOrderAmounts;
import com.hiqnet.pos.purchasing.application.PurchaseOrderDetail;
import com.hiqnet.pos.purchasing.application.PurchaseOrderService;
import com.hiqnet.pos.purchasing.application.UpdatePurchaseOrderCommand;
import com.hiqnet.pos.purchasing.application.UpdatePurchaseOrderLineCommand;
import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.Parameter;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.security.SecurityRequirement;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import java.math.BigDecimal;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * API administrativa de órdenes de compra de una sucursal.
 *
 * <p>Esta fundación administra únicamente borradores y sus partidas: no existen
 * todavía los endpoints de emisión ni de cancelación, y ninguna operación escribe
 * movimientos de inventario. Los errores usan el sobre estándar {@code ApiError}.</p>
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches/{branchId}/purchase-orders")
@Tag(name = "Purchasing Purchase Orders", description = "Órdenes de compra de una sucursal (borradores y partidas).")
public class PurchaseOrderController {

    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort ORDER = Sort.by(Sort.Order.desc("createdAt"), Sort.Order.desc("id"));

    private final PurchaseOrderService purchaseOrderService;

    public PurchaseOrderController(PurchaseOrderService purchaseOrderService) {
        this.purchaseOrderService = purchaseOrderService;
    }

    /**
     * Crea una orden de compra en borrador.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param request código y proveedor
     * @param jwt identidad del actor autenticado
     * @return orden creada con su detalle
     */
    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(
            summary = "Crear una orden de compra en borrador",
            description = "Crea la cabecera en estado DRAFT con la moneda de la organización y los snapshots del "
                    + "proveedor. El estado y la moneda no se aceptan desde el cliente. Permiso requerido: "
                    + "`purchasing.order.write`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Orden creada en borrador"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / INVALID_CODE / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.order.write` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organización, sucursal o proveedor inexistente o ajeno (ORGANIZATION_NOT_FOUND / BRANCH_NOT_FOUND / SUPPLIER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "BRANCH_INACTIVE / SUPPLIER_INACTIVE / PURCHASE_ORDER_CODE_ALREADY_EXISTS", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'purchasing.order.write')")
    public PurchaseOrderDetailResponse create(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @Valid @RequestBody CreatePurchaseOrderRequest request,
            @AuthenticationPrincipal Jwt jwt) {
        return PurchaseOrderDetailResponse.from(purchaseOrderService.create(
                organizationId, branchId, request.code(), request.supplierId(), actorId(jwt)));
    }

    /**
     * Lista las órdenes de compra de la sucursal.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino
     * @param page índice de página base cero
     * @param size elementos por página
     * @param status filtro de estado opcional
     * @param supplierId filtro de proveedor opcional
     * @param q texto opcional sobre el código
     * @return página de órdenes
     */
    @GetMapping
    @Operation(
            summary = "Listar órdenes de compra de la sucursal",
            description = "Paginado con orden fijo por fecha de creación descendente. Filtros opcionales `status`, "
                    + "`supplierId` y `q` (código). Permiso requerido: `purchasing.order.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Página de órdenes"),
        @ApiResponse(responseCode = "400", description = "Parámetros de paginación inválidos (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.order.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organización o sucursal inexistente o ajena", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'purchasing.order.read')")
    public PageResponse<PurchaseOrderResponse> list(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @Parameter(description = "Página base cero.", example = "0") @RequestParam(defaultValue = "0") int page,
            @Parameter(description = "Tamaño entre 1 y 100.", example = "20") @RequestParam(defaultValue = "20") int size,
            @Parameter(description = "Estado operativo opcional.", example = "DRAFT") @RequestParam(required = false) PurchaseOrderStatus status,
            @Parameter(description = "Proveedor opcional.", example = "f4a0e638-d4ce-4f84-aa98-9d2a56e1dd3e") @RequestParam(required = false) UUID supplierId,
            @Parameter(description = "Texto de búsqueda opcional sobre el código.", example = "po-2026") @RequestParam(required = false) String q) {
        validatePaging(page, size);
        Page<PurchaseOrder> orders = purchaseOrderService.list(
                organizationId, branchId, status, supplierId, q, PageRequest.of(page, size, ORDER));
        return PageResponse.from(orders, PurchaseOrderResponse::from);
    }

    /**
     * Obtiene el detalle de una orden con sus partidas.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @return detalle de la orden
     */
    @GetMapping("/{purchaseOrderId}")
    @Operation(
            summary = "Consultar el detalle de una orden de compra",
            description = "Incluye la cabecera y sus partidas. Permiso requerido: `purchasing.order.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Detalle de la orden"),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.order.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden inexistente o ajena (PURCHASE_ORDER_NOT_FOUND)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'purchasing.order.read')")
    public PurchaseOrderDetailResponse get(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId) {
        return PurchaseOrderDetailResponse.from(
                purchaseOrderService.get(organizationId, branchId, purchaseOrderId));
    }

    /**
     * Actualiza la cabecera de un borrador.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @param request nuevo proveedor opcional y versión esperada
     * @return orden actualizada
     */
    @PatchMapping("/{purchaseOrderId}")
    @Operation(
            summary = "Actualizar la cabecera de un borrador",
            description = "Solo editable en estado DRAFT. `expectedVersion` evita actualizaciones perdidas (409 "
                    + "CONCURRENT_MODIFICATION). El código, la moneda y el estado no pueden modificarse. Permiso "
                    + "requerido: `purchasing.order.write`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Orden actualizada"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.order.write` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden o proveedor inexistente o ajeno", content = @Content),
        @ApiResponse(responseCode = "409", description = "PURCHASE_ORDER_NOT_DRAFT / SUPPLIER_INACTIVE / CONCURRENT_MODIFICATION", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'purchasing.order.write')")
    public PurchaseOrderDetailResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId,
            @Valid @RequestBody UpdatePurchaseOrderRequest request) {
        return PurchaseOrderDetailResponse.from(purchaseOrderService.update(
                organizationId,
                branchId,
                purchaseOrderId,
                new UpdatePurchaseOrderCommand(request.expectedVersion(), request.supplierId())));
    }

    /**
     * Agrega una partida a un borrador.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @param request artículo, cantidad, costo opcional y versión esperada
     * @return orden con la nueva partida
     */
    @PostMapping("/{purchaseOrderId}/lines")
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(
            summary = "Agregar una partida a un borrador",
            description = "Captura los snapshots de código, nombre y unidad del artículo. Un artículo solo puede "
                    + "aparecer una vez por orden. Solo editable en estado DRAFT. Permiso requerido: "
                    + "`purchasing.order.write`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Partida agregada"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / INVALID_INVENTORY_QUANTITY / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.order.write` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden o artículo inexistente o ajeno (PURCHASE_ORDER_NOT_FOUND / INVENTORY_ITEM_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "PURCHASE_ORDER_NOT_DRAFT / PURCHASE_ORDER_LINE_DUPLICATE_ITEM / INVENTORY_ITEM_INACTIVE / CONCURRENT_MODIFICATION", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'purchasing.order.write')")
    public PurchaseOrderDetailResponse addLine(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId,
            @Valid @RequestBody CreatePurchaseOrderLineRequest request) {
        return PurchaseOrderDetailResponse.from(purchaseOrderService.addLine(
                organizationId,
                branchId,
                purchaseOrderId,
                new CreatePurchaseOrderLineCommand(
                        request.expectedVersion(),
                        request.inventoryItemId(),
                        PurchaseOrderAmounts.requireQuantity(request.quantity()),
                        PurchaseOrderAmounts.optionalUnitCost(request.unitCost()))));
    }

    /**
     * Actualiza la cantidad y/o el costo de una partida de un borrador.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @param purchaseOrderLineId identificador de la partida
     * @param request cantidad y costo opcionales y versión esperada
     * @return orden con la partida actualizada
     */
    @PatchMapping("/{purchaseOrderId}/lines/{purchaseOrderLineId}")
    @Operation(
            summary = "Actualizar una partida de un borrador",
            description = "Actualiza parcialmente cantidad y/o costo unitario; un campo ausente no cambia y "
                    + "`unitCost: null` limpia el costo. El artículo y sus snapshots son inmutables. Solo editable "
                    + "en estado DRAFT. Permiso requerido: `purchasing.order.write`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Partida actualizada"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / INVALID_INVENTORY_QUANTITY / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.order.write` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden o partida inexistente o ajena (PURCHASE_ORDER_NOT_FOUND / PURCHASE_ORDER_LINE_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "PURCHASE_ORDER_NOT_DRAFT / CONCURRENT_MODIFICATION", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'purchasing.order.write')")
    public PurchaseOrderDetailResponse updateLine(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId,
            @PathVariable UUID purchaseOrderLineId,
            @Valid @RequestBody UpdatePurchaseOrderLineRequest request) {
        BigDecimal quantity = request.quantity() == null
                ? null
                : PurchaseOrderAmounts.requireQuantity(request.quantity());
        boolean unitCostSpecified = request.unitCost() != null;
        BigDecimal unitCost = unitCostSpecified ? PurchaseOrderAmounts.optionalUnitCost(request.unitCost()) : null;
        return PurchaseOrderDetailResponse.from(purchaseOrderService.updateLine(
                organizationId,
                branchId,
                purchaseOrderId,
                purchaseOrderLineId,
                new UpdatePurchaseOrderLineCommand(request.expectedVersion(), quantity, unitCost, unitCostSpecified)));
    }

    /**
     * Elimina una partida de un borrador.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @param purchaseOrderLineId identificador de la partida
     * @param request versión esperada
     * @return orden sin la partida
     */
    @PostMapping("/{purchaseOrderId}/lines/{purchaseOrderLineId}/remove")
    @Operation(
            summary = "Eliminar una partida de un borrador",
            description = "Elimina físicamente la partida, solo mientras la orden está en DRAFT (nada la referencia "
                    + "todavía). Requiere `expectedVersion`. Permiso requerido: `purchasing.order.write`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Partida eliminada"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.order.write` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden o partida inexistente o ajena (PURCHASE_ORDER_NOT_FOUND / PURCHASE_ORDER_LINE_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "PURCHASE_ORDER_NOT_DRAFT / CONCURRENT_MODIFICATION", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'purchasing.order.write')")
    public PurchaseOrderDetailResponse removeLine(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId,
            @PathVariable UUID purchaseOrderLineId,
            @Valid @RequestBody ExpectedPurchaseOrderVersionRequest request) {
        return PurchaseOrderDetailResponse.from(purchaseOrderService.removeLine(
                organizationId, branchId, purchaseOrderId, purchaseOrderLineId, request.expectedVersion()));
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }

    private static UUID actorId(Jwt jwt) {
        if (jwt == null || jwt.getSubject() == null) {
            throw new BusinessException(
                    HttpStatus.UNAUTHORIZED, ErrorCodes.AUTHENTICATION_REQUIRED, "Authenticated user is required");
        }
        try {
            return UUID.fromString(jwt.getSubject());
        } catch (IllegalArgumentException exception) {
            throw new BusinessException(
                    HttpStatus.UNAUTHORIZED, ErrorCodes.AUTHENTICATION_REQUIRED, "Authenticated user is required");
        }
    }
}
