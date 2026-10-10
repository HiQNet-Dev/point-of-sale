package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.application.ProductService;
import com.hiqnet.pos.catalog.application.UpdateProductCommand;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductStatus;
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

/**
 * Organization-scoped product administration. The catalog belongs to the
 * organization, so every endpoint authorizes with organization-scope
 * {@code hasPermission} (never {@code hasBranchPermission}). There is no
 * DELETE: a product is retired via status INACTIVE.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/products")
public class ProductController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: name ASC, id ASC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final ProductService productService;

    public ProductController(ProductService productService) {
        this.productService = productService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a product inside an organization (optional category, base unit price)")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or category not found (ORGANIZATION_NOT_FOUND, CATEGORY_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate sku/barcode or inactive category (PRODUCT_SKU_ALREADY_EXISTS, PRODUCT_BARCODE_ALREADY_EXISTS, CATEGORY_INACTIVE)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (product.create)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'product.create')")
    public ProductResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateProductRequest request) {
        return ProductResponse.from(productService.create(
                organizationId,
                request.sku(),
                request.barcode(),
                request.name(),
                request.description(),
                request.categoryId(),
                request.unitPrice()));
    }

    @GetMapping
    @Operation(summary = "List products of an organization (paginated; filters status, categoryId, q)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or category not found (ORGANIZATION_NOT_FOUND, CATEGORY_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (product.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'product.read')")
    public PageResponse<ProductResponse> list(
            @PathVariable UUID organizationId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) ProductStatus status,
            @RequestParam(required = false) UUID categoryId,
            @RequestParam(required = false) String q) {
        validatePaging(page, size);
        Page<Product> products =
                productService.list(organizationId, status, categoryId, q, PageRequest.of(page, size, ORDER));
        return PageResponse.from(products, ProductResponse::from);
    }

    @GetMapping("/{productId}")
    @Operation(summary = "Get a product by id (scoped to the organization)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or product not found (ORGANIZATION_NOT_FOUND, PRODUCT_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (product.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'product.read')")
    public ProductResponse get(@PathVariable UUID organizationId, @PathVariable UUID productId) {
        return ProductResponse.from(productService.get(organizationId, productId));
    }

    @PatchMapping("/{productId}")
    @Operation(summary = "Partially update a product (sku immutable; categoryId is tri-state)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, product or category not found (ORGANIZATION_NOT_FOUND, PRODUCT_NOT_FOUND, CATEGORY_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate barcode, inactive category or concurrent modification (PRODUCT_BARCODE_ALREADY_EXISTS, CATEGORY_INACTIVE, CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (product.update)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'product.update')")
    public ProductResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID productId,
            @Valid @RequestBody UpdateProductRequest request) {
        return ProductResponse.from(productService.update(
                organizationId, productId, new UpdateProductCommand(
                        request.expectedVersion(),
                        request.barcode(),
                        request.name(),
                        request.description(),
                        request.categoryId(),
                        request.unitPrice(),
                        request.status())));
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }
}
