package com.hiqnet.pos.membership.domain;

import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface BranchMembershipRoleRepository
        extends JpaRepository<BranchMembershipRole, BranchMembershipRoleId> {

    List<BranchMembershipRole> findByOrganizationIdAndMembershipId(UUID organizationId, UUID membershipId);

    @Modifying
    @Query("delete from BranchMembershipRole bmr "
            + "where bmr.organizationId = :organizationId and bmr.membershipId = :membershipId")
    void deleteByOrganizationIdAndMembershipId(
            @Param("organizationId") UUID organizationId, @Param("membershipId") UUID membershipId);

    /**
     * Database-authoritative branch-scoped permission check (single EXISTS, no
     * N+1, no cache). Grants only when ALL branch-specific conditions hold:
     * the user ACTIVE, the organization ACTIVE, the BRANCH ACTIVE and in the
     * organization, the MEMBERSHIP ACTIVE for that exact branch, the role
     * ACTIVE, in the same organization and assigned to the membership, and the
     * role contains the permission. That is the full §branch-scope predicate:
     * an INACTIVE branch, membership or role kills the grant immediately, with
     * no JWT change (authorization is PostgreSQL-authoritative).
     */
    @Query(
            value = """
                    select exists(
                      select 1
                      from branch_membership_roles bmr
                      join branch_memberships bm
                        on bm.id = bmr.membership_id and bm.organization_id = bmr.organization_id
                      join branches b
                        on b.id = bm.branch_id and b.organization_id = bm.organization_id
                      join users u
                        on u.id = bm.user_id and u.organization_id = bm.organization_id
                      join roles r
                        on r.id = bmr.role_id and r.organization_id = bmr.organization_id
                      join role_permissions rp on rp.role_id = r.id
                      where bm.organization_id = :organizationId
                        and bm.user_id = :userId
                        and bm.branch_id = :branchId
                        and bm.status = 'ACTIVE'
                        and b.status = 'ACTIVE'
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
    boolean existsEffectiveBranchPermission(
            @Param("userId") UUID userId,
            @Param("organizationId") UUID organizationId,
            @Param("branchId") UUID branchId,
            @Param("permissionCode") String permissionCode);

    /**
     * Branch-scoped permission check for RECOVERY operations (closing an
     * already-open cash shift) that must survive branch deactivation. Identical
     * to {@link #existsEffectiveBranchPermission} EXCEPT it does NOT require the
     * branch itself to be ACTIVE: the branch and organization must still exist
     * and match, and the membership, user and role must be ACTIVE. A branch
     * disabled administratively must not trap an OPEN shift behind a branch-scoped
     * grant. Opening still uses {@link #existsEffectiveBranchPermission}.
     */
    @Query(
            value = """
                    select exists(
                      select 1
                      from branch_membership_roles bmr
                      join branch_memberships bm
                        on bm.id = bmr.membership_id and bm.organization_id = bmr.organization_id
                      join branches b
                        on b.id = bm.branch_id and b.organization_id = bm.organization_id
                      join users u
                        on u.id = bm.user_id and u.organization_id = bm.organization_id
                      join roles r
                        on r.id = bmr.role_id and r.organization_id = bmr.organization_id
                      join role_permissions rp on rp.role_id = r.id
                      where bm.organization_id = :organizationId
                        and bm.user_id = :userId
                        and bm.branch_id = :branchId
                        and bm.status = 'ACTIVE'
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
    boolean existsBranchPermissionIgnoringBranchStatus(
            @Param("userId") UUID userId,
            @Param("organizationId") UUID organizationId,
            @Param("branchId") UUID branchId,
            @Param("permissionCode") String permissionCode);
}