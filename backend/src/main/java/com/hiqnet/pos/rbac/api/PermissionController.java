package com.hiqnet.pos.rbac.api;

import com.hiqnet.pos.rbac.application.PermissionService;
import com.hiqnet.pos.rbac.domain.Permission;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import java.util.List;
import java.util.UUID;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Read-only permission catalog. Permissions are NOT a client CRUD: they are
 * seeded by Flyway and only listed here.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/permissions")
public class PermissionController {

    private final PermissionService permissionService;

    public PermissionController(PermissionService permissionService) {
        this.permissionService = permissionService;
    }

    @GetMapping
    @Operation(summary = "List the system permission catalog (code ASC)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (permission.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'permission.read')")
    public List<PermissionResponse> list(@PathVariable UUID organizationId) {
        return permissionService.list().stream().map(PermissionResponse::from).toList();
    }
}