package com.hiqnet.pos.rbac.api;

import com.hiqnet.pos.rbac.application.RolePermissionService;
import com.hiqnet.pos.rbac.application.RoleService;
import com.hiqnet.pos.rbac.domain.Permission;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import java.util.List;
import java.util.UUID;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Permission set of an organization role. PUT replaces the complete set
 * atomically; the role is resolved scoped to the organization first, so a
 * cross-organization role id gets 404 before any mutation.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/roles/{roleId}/permissions")
@Tag(name = "RBAC", description = "Roles, permisos y sus asignaciones (autorización basada en permisos).")
public class RolePermissionController {

    private final RoleService roleService;
    private final RolePermissionService rolePermissionService;

    public RolePermissionController(RoleService roleService, RolePermissionService rolePermissionService) {
        this.roleService = roleService;
        this.rolePermissionService = rolePermissionService;
    }

    @GetMapping
    @Operation(summary = "List the permissions granted to a role (code ASC)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or role not found", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.read')")
    public List<PermissionResponse> list(@PathVariable UUID organizationId, @PathVariable UUID roleId) {
        roleService.get(organizationId, roleId);
        return toResponses(rolePermissionService.list(roleId));
    }

    @PutMapping
    @Operation(summary = "Atomically replace the role's permission set")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Replaced; returns the new set", content = @Content),
        @ApiResponse(responseCode = "400", description = "Duplicate permission codes (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, role or permission not found", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.permissions.manage)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.permissions.manage')")
    public List<PermissionResponse> replace(
            @PathVariable UUID organizationId,
            @PathVariable UUID roleId,
            @Valid @RequestBody PutRolePermissionsRequest request) {
        roleService.get(organizationId, roleId);
        rolePermissionService.replace(organizationId, roleId, request.permissions());
        return toResponses(rolePermissionService.list(roleId));
    }

    private static List<PermissionResponse> toResponses(List<Permission> permissions) {
        return permissions.stream().map(PermissionResponse::from).toList();
    }
}