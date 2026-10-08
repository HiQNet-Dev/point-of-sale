package com.hiqnet.pos.rbac.domain;

import java.io.Serializable;
import java.util.UUID;

/** Composite key of {@link UserOrganizationRole}. */
public record UserOrganizationRoleId(UUID organizationId, UUID userId, UUID roleId) implements Serializable {

    public UserOrganizationRoleId {
        if (organizationId == null || userId == null || roleId == null) {
            throw new IllegalArgumentException("organizationId, userId and roleId are required");
        }
    }
}