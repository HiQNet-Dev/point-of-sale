package com.hiqnet.pos.rbac.domain;

import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface UserOrganizationRoleRepository
        extends JpaRepository<UserOrganizationRole, UserOrganizationRoleId> {

    List<UserOrganizationRole> findByOrganizationIdAndUserId(UUID organizationId, UUID userId);

    @Modifying
    @Query("delete from UserOrganizationRole uor "
            + "where uor.organizationId = :organizationId and uor.userId = :userId")
    void deleteByOrganizationIdAndUserId(
            @Param("organizationId") UUID organizationId, @Param("userId") UUID userId);

    /**
     * Database-authoritative check that the user currently holds the effective
     * permission. PostgreSQL is the authority (JWT is identity-only): user and
     * organization must be ACTIVE, the user must belong to the organization,
     * the role must be ACTIVE and assigned, and the role must contain the
     * permission. Single EXISTS query, no N+1, no cache.
     */
    @Query(
            value = """
                    select exists(
                      select 1
                      from user_organization_roles uor
                      join users u on u.id = uor.user_id and u.organization_id = uor.organization_id
                      join roles r on r.id = uor.role_id and r.organization_id = uor.organization_id
                      join role_permissions rp on rp.role_id = r.id
                      where uor.organization_id = :organizationId
                        and uor.user_id = :userId
                        and u.status = 'ACTIVE'
                        and r.status = 'ACTIVE'
                        and rp.permission_code = :permissionCode
                        and exists (
                            select 1 from organizations o
                            where o.id = :organizationId and o.status = 'ACTIVE'
                        )
                    )
                    """,
            nativeQuery = true)
    boolean existsEffectivePermission(
            @Param("userId") UUID userId,
            @Param("organizationId") UUID organizationId,
            @Param("permissionCode") String permissionCode);
}