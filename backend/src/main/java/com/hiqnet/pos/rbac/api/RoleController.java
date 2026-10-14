package com.hiqnet.pos.rbac.api;

import com.hiqnet.pos.rbac.application.RoleService;
import com.hiqnet.pos.rbac.application.UpdateRoleCommand;
import com.hiqnet.pos.rbac.domain.RoleStatus;
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
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;

/**
 * Organization-scoped role administration. No DELETE: deactivation is
 * {@code status = INACTIVE}. Permission checks are database-authoritative
 * (JWT is identity-only).
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/roles")
@Tag(name = "RBAC", description = "Roles, permisos y sus asignaciones (autorización basada en permisos).")
public class RoleController {

    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort ORDER = Sort.by(Sort.Order.asc("code"), Sort.Order.asc("id"));

    private final RoleService roleService;

    public RoleController(RoleService roleService) {
        this.roleService = roleService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a role inside an organization")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation error (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (ROLE_CODE_ALREADY_EXISTS)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.create)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.create')")
    public RoleResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateRoleRequest request) {
        return RoleResponse.from(roleService.create(organizationId, request.code(), request.name()));
    }

    @GetMapping
    @Operation(summary = "List roles of an organization (paginated, ordered by code)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.read')")
    public PageResponse<RoleResponse> list(
            @PathVariable UUID organizationId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) RoleStatus status) {
        validatePaging(page, size);
        Page<com.hiqnet.pos.rbac.domain.Role> roles =
                roleService.list(organizationId, status, PageRequest.of(page, size, ORDER));
        return PageResponse.from(roles, RoleResponse::from);
    }

    @GetMapping("/{roleId}")
    @Operation(summary = "Get a role by id (scoped to the organization)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or role not found", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.read')")
    public RoleResponse get(@PathVariable UUID organizationId, @PathVariable UUID roleId) {
        return RoleResponse.from(roleService.get(organizationId, roleId));
    }

    @PatchMapping("/{roleId}")
    @Operation(summary = "Partially update a role (name and/or status; code immutable)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation error (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or role not found", content = @Content),
        @ApiResponse(responseCode = "409", description = "Stale or concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.update)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.update')")
    public RoleResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID roleId,
            @Valid @RequestBody UpdateRoleRequest request) {
        return RoleResponse.from(roleService.update(
                organizationId,
                roleId,
                new UpdateRoleCommand(request.expectedVersion(), request.name(), request.status())));
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