package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.application.PurchaseOrderAmounts;
import com.hiqnet.pos.purchasing.application.PurchaseReceiptLineCommand;
import com.hiqnet.pos.purchasing.application.PurchaseReceiptOutcome;
import com.hiqnet.pos.purchasing.application.PurchaseReceiptService;
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
import java.util.List;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * API de recepción de mercancía de órdenes de compra.
 *
 * <p>Registra recepciones parciales o completas con actualización atómica de
 * existencias (movimientos {@code PURCHASE_RECEIPT} append-only), idempotencia
 * estricta por clave y auditoría transaccional. Consulta la recepción, el listado
 * de recepciones de una orden y las cantidades pendientes por partida. Los errores
 * usan el sobre estándar {@code ApiError}.</p>
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches/{branchId}/purchase-orders/{purchaseOrderId}")
@Tag(name = "Purchasing Purchase Receipts", description = "Recepción de mercancía de órdenes de compra.")
public class PurchaseReceiptController {

    private final PurchaseReceiptService purchaseReceiptService;

    public PurchaseReceiptController(PurchaseReceiptService purchaseReceiptService) {
        this.purchaseReceiptService = purchaseReceiptService;
    }

    /**
     * Registra una recepción (parcial o completa) de una orden emitida.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @param request clave de idempotencia, versión esperada y líneas recibidas
     * @param jwt identidad del actor autenticado
     * @return recepción confirmada (201) o el resultado original en un reintento idéntico (200)
     */
    @PostMapping("/receipts")
    @Operation(
            summary = "Registrar una recepción de mercancía",
            description = "Confirma stock real de una orden emitida en una sola transacción. Exige una "
                    + "`idempotencyKey` única por organización y orden: un reintento con el mismo contenido devuelve "
                    + "el resultado original sin nuevos efectos; con contenido distinto responde 409. Rechaza "
                    + "sobrerrecepción y órdenes DRAFT/CANCELLED/RECEIVED. Permiso requerido: "
                    + "`purchasing.receipt.write`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Recepción registrada"),
        @ApiResponse(responseCode = "200", description = "Reintento idéntico: devuelve el resultado original"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / INVALID_INVENTORY_QUANTITY / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.receipt.write` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden, partida, artículo o sucursal inexistente o ajena", content = @Content),
        @ApiResponse(responseCode = "409", description = "PURCHASE_ORDER_NOT_ISSUED / PURCHASE_ORDER_ALREADY_RECEIVED / PURCHASE_ORDER_ALREADY_CANCELLED / PURCHASE_RECEIPT_EXCEEDS_ORDERED / PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT / INVENTORY_ITEM_INACTIVE / PURCHASE_ORDER_UNIT_CHANGED / BRANCH_INACTIVE / CONCURRENT_MODIFICATION", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'purchasing.receipt.write')")
    public ResponseEntity<PurchaseReceiptDetailResponse> post(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId,
            @Valid @RequestBody PostPurchaseReceiptRequest request,
            @AuthenticationPrincipal Jwt jwt) {
        List<PurchaseReceiptLineCommand> lines = request.lines().stream()
                .map(line -> new PurchaseReceiptLineCommand(
                        line.purchaseOrderLineId(), PurchaseOrderAmounts.requireQuantity(line.quantity())))
                .toList();
        PurchaseReceiptOutcome outcome = purchaseReceiptService.post(
                organizationId,
                branchId,
                purchaseOrderId,
                request.idempotencyKey(),
                request.expectedVersion(),
                actorId(jwt),
                lines);
        HttpStatus status = outcome.replayed() ? HttpStatus.OK : HttpStatus.CREATED;
        return ResponseEntity.status(status).body(PurchaseReceiptDetailResponse.from(outcome.detail()));
    }

    /**
     * Lista las recepciones confirmadas de una orden.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @return recepciones ordenadas cronológicamente
     */
    @GetMapping("/receipts")
    @Operation(
            summary = "Listar las recepciones de una orden",
            description = "Recepciones confirmadas ordenadas por fecha. Permiso requerido: `purchasing.receipt.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Listado de recepciones"),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.receipt.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden inexistente o ajena (PURCHASE_ORDER_NOT_FOUND)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'purchasing.receipt.read')")
    public List<PurchaseReceiptResponse> list(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId) {
        return purchaseReceiptService.list(organizationId, branchId, purchaseOrderId).stream()
                .map(PurchaseReceiptResponse::from)
                .toList();
    }

    /**
     * Obtiene una recepción con sus líneas y la orden afectada.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @param purchaseReceiptId identificador de la recepción
     * @return detalle de la recepción
     */
    @GetMapping("/receipts/{purchaseReceiptId}")
    @Operation(
            summary = "Consultar una recepción",
            description = "Incluye la cabecera, sus líneas y el estado vigente de la orden. Permiso requerido: "
                    + "`purchasing.receipt.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Detalle de la recepción"),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.receipt.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden o recepción inexistente o ajena (PURCHASE_ORDER_NOT_FOUND / PURCHASE_RECEIPT_NOT_FOUND)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'purchasing.receipt.read')")
    public PurchaseReceiptDetailResponse get(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID purchaseOrderId,
            @PathVariable UUID purchaseReceiptId) {
        return PurchaseReceiptDetailResponse.from(
                purchaseReceiptService.get(organizationId, branchId, purchaseOrderId, purchaseReceiptId));
    }

    /**
     * Consulta las cantidades recibidas y pendientes por partida.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino
     * @param purchaseOrderId identificador de la orden
     * @return estado de recepción por partida
     */
    @GetMapping("/receipt-status")
    @Operation(
            summary = "Consultar el estado de recepción de una orden",
            description = "Cantidades solicitadas, recibidas y pendientes por partida. Permiso requerido: "
                    + "`purchasing.receipt.read`.")
    @SecurityRequirement(name = "bearerAuth")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Estado de recepción por partida"),
        @ApiResponse(responseCode = "401", description = "AUTHENTICATION_REQUIRED / TOKEN_INVALID / TOKEN_EXPIRED", content = @Content),
        @ApiResponse(responseCode = "403", description = "Sin el permiso `purchasing.receipt.read` (FORBIDDEN)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Orden inexistente o ajena (PURCHASE_ORDER_NOT_FOUND)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'purchasing.receipt.read')")
    public PurchaseOrderReceiptStatusResponse receiptStatus(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @Parameter(description = "Identificador de la orden.") @PathVariable UUID purchaseOrderId) {
        return PurchaseOrderReceiptStatusResponse.from(
                purchaseReceiptService.receiptStatus(organizationId, branchId, purchaseOrderId));
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
