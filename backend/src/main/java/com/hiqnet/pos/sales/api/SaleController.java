package com.hiqnet.pos.sales.api;

import com.hiqnet.pos.payment.api.PaymentResponse;
import com.hiqnet.pos.sales.application.SaleService;
import com.hiqnet.pos.sales.domain.Sale;
import com.hiqnet.pos.sales.domain.SaleStatus;
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
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/** Branch-scoped operational sale API. No payment, completion or physical delete endpoint exists. */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts/{cashShiftId}/sales")
public class SaleController {
    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort ORDER = Sort.by(Sort.Order.desc("createdAt"), Sort.Order.desc("id"));
    private final SaleService saleService;

    public SaleController(SaleService saleService) { this.saleService = saleService; }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'sale.create')")
    public SaleResponse create(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @AuthenticationPrincipal Jwt jwt) {
        return SaleResponse.from(saleService.create(organizationId, branchId, cashRegisterId, cashShiftId, actorId(jwt)));
    }

    @GetMapping
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'sale.read')")
    public PageResponse<SaleResponse> list(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId,
            @RequestParam(defaultValue = "0") int page, @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) SaleStatus status) {
        validatePaging(page, size);
        Page<Sale> sales = saleService.list(organizationId, branchId, cashRegisterId, cashShiftId, status,
                PageRequest.of(page, size, ORDER));
        return PageResponse.from(sales, SaleResponse::from);
    }

    @GetMapping("/{saleId}")
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'sale.read')")
    public SaleDetailResponse get(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @PathVariable UUID saleId) {
        return SaleDetailResponse.from(saleService.get(organizationId, branchId, cashRegisterId, cashShiftId, saleId));
    }

    @PostMapping("/{saleId}/checkout")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'sale.checkout')")
    public SaleDetailResponse checkout(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @PathVariable UUID saleId,
            @AuthenticationPrincipal Jwt jwt, @Valid @RequestBody CheckoutRequest request) {
        return SaleDetailResponse.from(saleService.checkout(organizationId, branchId, cashRegisterId, cashShiftId, saleId,
                request.expectedSaleVersion(), request.payments(), actorId(jwt)));
    }

    @GetMapping("/{saleId}/payments")
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'payment.read')")
    public java.util.List<PaymentResponse> payments(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @PathVariable UUID saleId) {
        return saleService.payments(organizationId, branchId, cashRegisterId, cashShiftId, saleId).stream()
                .map(PaymentResponse::from).toList();
    }

    @PostMapping("/{saleId}/items")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'sale.update')")
    public SaleDetailResponse addItem(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @PathVariable UUID saleId,
            @AuthenticationPrincipal Jwt jwt, @Valid @RequestBody AddSaleItemRequest request) {
        return SaleDetailResponse.from(saleService.addItem(organizationId, branchId, cashRegisterId, cashShiftId, saleId,
                request.expectedSaleVersion(), request.productId(), request.quantity(), request.modifierOptionIds(), actorId(jwt)));
    }

    @PatchMapping("/{saleId}/items/{saleItemId}")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'sale.update')")
    public SaleDetailResponse updateItem(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @PathVariable UUID saleId,
            @PathVariable UUID saleItemId, @AuthenticationPrincipal Jwt jwt, @Valid @RequestBody UpdateSaleItemRequest request) {
        return SaleDetailResponse.from(saleService.updateItem(organizationId, branchId, cashRegisterId, cashShiftId, saleId,
                saleItemId, request.expectedSaleVersion(), request.quantity(), request.modifierOptionIds(), actorId(jwt)));
    }

    @PostMapping("/{saleId}/items/{saleItemId}/remove")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'sale.update')")
    public SaleDetailResponse removeItem(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @PathVariable UUID saleId,
            @PathVariable UUID saleItemId, @AuthenticationPrincipal Jwt jwt, @Valid @RequestBody ExpectedSaleVersionRequest request) {
        return SaleDetailResponse.from(saleService.removeItem(organizationId, branchId, cashRegisterId, cashShiftId, saleId,
                saleItemId, request.expectedVersion(), actorId(jwt)));
    }

    @PostMapping("/{saleId}/cancel")
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'sale.cancel')")
    public SaleDetailResponse cancel(@PathVariable UUID organizationId, @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId, @PathVariable UUID cashShiftId, @PathVariable UUID saleId,
            @AuthenticationPrincipal Jwt jwt, @Valid @RequestBody ExpectedSaleVersionRequest request) {
        return SaleDetailResponse.from(saleService.cancel(organizationId, branchId, cashRegisterId, cashShiftId, saleId,
                request.expectedVersion(), actorId(jwt)));
    }

    private static UUID actorId(Jwt jwt) {
        if (jwt == null || jwt.getSubject() == null) throw new BusinessException(HttpStatus.UNAUTHORIZED, ErrorCodes.AUTHENTICATION_REQUIRED, "Authenticated user is required");
        try { return UUID.fromString(jwt.getSubject()); }
        catch (IllegalArgumentException exception) { throw new BusinessException(HttpStatus.UNAUTHORIZED, ErrorCodes.AUTHENTICATION_REQUIRED, "Authenticated user is required"); }
    }

    private static void validatePaging(int page, int size) {
        if (page < 0 || size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0 and size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }
}
