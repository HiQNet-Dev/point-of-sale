package com.hiqnet.pos.rbac.domain;

import java.io.Serializable;
import java.util.UUID;

/** Composite key of {@link RolePermission}. */
public record RolePermissionId(UUID roleId, String permissionCode) implements Serializable {

    public RolePermissionId {
        if (roleId == null || permissionCode == null) {
            throw new IllegalArgumentException("roleId and permissionCode are required");
        }
    }
}