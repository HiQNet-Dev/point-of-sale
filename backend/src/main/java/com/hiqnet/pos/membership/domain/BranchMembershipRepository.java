package com.hiqnet.pos.membership.domain;

import jakarta.persistence.LockModeType;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface BranchMembershipRepository extends JpaRepository<BranchMembership, UUID> {

    /** Scoped lookup: a membership is never resolved by a global id alone. */
    Optional<BranchMembership> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /**
     * Natural lookup: one user has AT MOST one membership per branch (unique
     * constraint (organization_id, branch_id, user_id)).
     */
    Optional<BranchMembership> findByOrganizationIdAndBranchIdAndUserId(
            UUID organizationId, UUID branchId, UUID userId);

    /**
     * Scoped locking lookup for administrative mutations (status changes and
     * role replacement). Acquires the membership row first (PESSIMISTIC_WRITE)
     * so the subsequent {@code branch_membership_roles} mutation keeps the
     * fixed lock order membership -> membership_roles (mirrors the auth rule
     * user -> sessions; never the other way around). Organization scoping
     * (IDOR) is preserved.
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("""
            select bm from BranchMembership bm
            where bm.organizationId = :organizationId
              and bm.branchId = :branchId
              and bm.userId = :userId
            """)
    Optional<BranchMembership> findByOrganizationIdAndBranchIdAndUserIdForUpdate(
            @Param("organizationId") UUID organizationId,
            @Param("branchId") UUID branchId,
            @Param("userId") UUID userId);

    boolean existsByOrganizationIdAndBranchIdAndUserId(UUID organizationId, UUID branchId, UUID userId);

    Page<BranchMembership> findByOrganizationIdAndBranchId(
            UUID organizationId, UUID branchId, Pageable pageable);

    Page<BranchMembership> findByOrganizationIdAndBranchIdAndStatus(
            UUID organizationId, UUID branchId, BranchMembershipStatus status, Pageable pageable);

    /**
     * Self-discovery projection: the branch ids the user is ACTIVELY a member of
     * inside one organization. Only {@code membership.status = ACTIVE} is
     * required — branch status and role state are deliberately NOT filtered, so
     * a branch that was deactivated remains discoverable to its members
     * (recovery), and discovery is never an authorization decision.
     */
    @Query("""
            select bm.branchId from BranchMembership bm
            where bm.organizationId = :organizationId
              and bm.userId = :userId
              and bm.status = :status
            """)
    List<UUID> findBranchIdsByOrganizationIdAndUserIdAndStatus(
            @Param("organizationId") UUID organizationId,
            @Param("userId") UUID userId,
            @Param("status") BranchMembershipStatus status);
}