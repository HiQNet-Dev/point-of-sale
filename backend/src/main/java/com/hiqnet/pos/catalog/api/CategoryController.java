package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.application.CategoryService;
import com.hiqnet.pos.catalog.application.UpdateCategoryCommand;
import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;
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
 * Organization-scoped category administration. The catalog belongs to the
 * organization, so every endpoint authorizes with organization-scope
 * {@code hasPermission} (never {@code hasBranchPermission}). There is no
 * DELETE: a category is retired via status INACTIVE.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/categories")
@Tag(name = "Catalog", description = "Catálogo de la organización: categorías y productos.")
public class CategoryController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: name ASC, id ASC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final CategoryService categoryService;

    public CategoryController(CategoryService categoryService) {
        this.categoryService = categoryService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a category inside an organization")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR, INVALID_CODE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (CATEGORY_CODE_ALREADY_EXISTS)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (category.create)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'category.create')")
    public CategoryResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateCategoryRequest request) {
        return CategoryResponse.from(categoryService.create(organizationId, request.code(), request.name()));
    }

    @GetMapping
    @Operation(summary = "List categories of an organization (paginated, ordered by name)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (category.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'category.read')")
    public PageResponse<CategoryResponse> list(
            @PathVariable UUID organizationId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) CategoryStatus status) {
        validatePaging(page, size);
        Page<Category> categories =
                categoryService.list(organizationId, status, PageRequest.of(page, size, ORDER));
        return PageResponse.from(categories, CategoryResponse::from);
    }

    @GetMapping("/{categoryId}")
    @Operation(summary = "Get a category by id (scoped to the organization)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or category not found (ORGANIZATION_NOT_FOUND, CATEGORY_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (category.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'category.read')")
    public CategoryResponse get(@PathVariable UUID organizationId, @PathVariable UUID categoryId) {
        return CategoryResponse.from(categoryService.get(organizationId, categoryId));
    }

    @PatchMapping("/{categoryId}")
    @Operation(summary = "Partially update a category (name and/or status; code is immutable)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or category not found (ORGANIZATION_NOT_FOUND, CATEGORY_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (category.update)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'category.update')")
    public CategoryResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID categoryId,
            @Valid @RequestBody UpdateCategoryRequest request) {
        return CategoryResponse.from(categoryService.update(
                organizationId, categoryId, new UpdateCategoryCommand(
                        request.expectedVersion(), request.name(), request.status())));
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
