package com.hiqnet.pos.rbac.api;

import com.hiqnet.pos.rbac.domain.Permission;

/** Public view of one catalog permission. */
public record PermissionResponse(String code, String description) {

    public static PermissionResponse from(Permission permission) {
        return new PermissionResponse(permission.getCode(), permission.getDescription());
    }
}