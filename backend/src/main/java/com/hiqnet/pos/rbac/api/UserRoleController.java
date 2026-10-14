package com.hiqnet.pos.rbac.api;

import com.hiqnet.pos.rbac.application.UserRoleService;
import com.hiqnet.pos.rbac.domain.Role;
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
 * Organization-wide role assignments of a user. PUT replaces the complete set
 * atomically; every role must belong to the same organization (cross-tenant
 * ids fail with 404 before any mutation; PostgreSQL composite FKs are the
 * final barrier).
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/users/{userId}/roles")
@Tag(name = "RBAC", description = "Roles, permisos y sus asignaciones (autorización basada en permisos).")
public class UserRoleController {

    private final UserRoleService userRoleService;

    public UserRoleController(UserRoleService userRoleService) {
        this.userRoleService = userRoleService;
    }

    @GetMapping
    @Operation(summary = "List the organization roles assigned to a user")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or user not found", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.read')")
    public List<RoleResponse> list(@PathVariable UUID organizationId, @PathVariable UUID userId) {
        return userRoleService.list(organizationId, userId).stream().map(RoleResponse::from).toList();
    }

    @PutMapping
    @Operation(summary = "Atomically replace the user's organization role set")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Replaced; returns the new set", content = @Content),
        @ApiResponse(responseCode = "400", description = "Duplicate role ids (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, user or role not found", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (role.assign)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'role.assign')")
    public List<RoleResponse> replace(
            @PathVariable UUID organizationId,
            @PathVariable UUID userId,
            @Valid @RequestBody PutUserRolesRequest request) {
        userRoleService.replace(organizationId, userId, request.roleIds());
        return userRoleService.list(organizationId, userId).stream().map(RoleResponse::from).toList();
    }
}