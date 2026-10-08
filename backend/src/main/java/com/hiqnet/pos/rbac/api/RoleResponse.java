package com.hiqnet.pos.rbac.api;

import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleStatus;
import java.time.Instant;
import java.util.UUID;

/** Administrative view of an organization role. */
public record RoleResponse(
        UUID id,
        UUID organizationId,
        String code,
        String name,
        RoleStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static RoleResponse from(Role role) {
        return new RoleResponse(
                role.getId(),
                role.getOrganizationId(),
                role.getCode(),
                role.getName(),
                role.getStatus(),
                role.getVersion(),
                role.getCreatedAt(),
                role.getUpdatedAt());
    }
}