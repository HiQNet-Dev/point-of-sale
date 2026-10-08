package com.hiqnet.pos.rbac.domain;

import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface RolePermissionRepository extends JpaRepository<RolePermission, RolePermissionId> {

    List<RolePermission> findByRoleId(UUID roleId);

    @Modifying
    @Query("delete from RolePermission rp where rp.roleId = :roleId")
    void deleteByRoleId(@Param("roleId") UUID roleId);
}