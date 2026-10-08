package com.hiqnet.pos.rbac.api;

import com.hiqnet.pos.rbac.domain.RoleStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

/**
 * Partial update: {@code name} and/or {@code status}; {@code code} is
 * immutable. {@code expectedVersion} is required (the version the client read):
 * a mismatch returns 409 CONCURRENT_MODIFICATION before any change.
 */
public record UpdateRoleRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "3")
        @NotNull
                @Min(0)
                Long expectedVersion,
        @Size(max = 120) String name,
        RoleStatus status) {
}