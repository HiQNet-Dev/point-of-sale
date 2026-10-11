package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.application.OperationalCatalogService;
import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryStatus;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;

import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.validation.annotation.Validated;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

/** Read-only POS catalog API; Branch is the authorization context, not catalog ownership. */
@RestController
@Validated
@RequestMapping("/api/v1/organizations/{organizationId}/branches/{branchId}/pos/catalog")
public class OperationalCatalogController {

    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final OperationalCatalogService operationalCatalogService;

    public OperationalCatalogController(OperationalCatalogService operationalCatalogService) {
        this.operationalCatalogService = operationalCatalogService;
    }

    @GetMapping("/categories")
    @Operation(summary = "List organization categories in an active branch POS context")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'pos.catalog.read')")
    public PageResponse<PosCategoryResponse> categories(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) CategoryStatus status) {
        validatePaging(page, size);
        Page<Category> categories = operationalCatalogService.categories(
                organizationId, branchId, status, PageRequest.of(page, size, ORDER));
        return PageResponse.from(categories, PosCategoryResponse::from);
    }

    @GetMapping("/products")
    @Operation(summary = "List active sellable candidate products in an active branch POS context")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'pos.catalog.read')")
    public PageResponse<PosProductResponse> products(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) UUID categoryId,
            @RequestParam(required = false) String q) {
        validatePaging(page, size);
        Page<Product> products = operationalCatalogService.products(
                organizationId, branchId, categoryId, q, PageRequest.of(page, size, ORDER));
        return PageResponse.from(products, PosProductResponse::from);
    }

    @GetMapping("/products/{productId}/configuration")
    @Operation(summary = "Read the current assigned modifier configuration for an active product")
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'pos.catalog.read')")
    public PosProductConfigurationResponse configuration(
            @PathVariable UUID organizationId, @PathVariable UUID branchId, @PathVariable UUID productId) {
        return operationalCatalogService.configuration(organizationId, branchId, productId);
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
}
