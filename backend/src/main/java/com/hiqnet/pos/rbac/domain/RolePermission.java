package com.hiqnet.pos.rbac.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.IdClass;
import jakarta.persistence.Table;
import java.util.UUID;

/** Grant of one global permission to an organization role. */
@Entity
@Table(name = "role_permissions")
@IdClass(RolePermissionId.class)
public class RolePermission {

    @Id
    @Column(name = "role_id", nullable = false)
    private UUID roleId;

    @Id
    @Column(name = "permission_code", nullable = false, length = 64)
    private String permissionCode;

    protected RolePermission() {
    }

    private RolePermission(UUID roleId, String permissionCode) {
        this.roleId = roleId;
        this.permissionCode = permissionCode;
    }

    public static RolePermission of(UUID roleId, String permissionCode) {
        return new RolePermission(roleId, permissionCode);
    }

    public UUID getRoleId() {
        return roleId;
    }

    public String getPermissionCode() {
        return permissionCode;
    }
}