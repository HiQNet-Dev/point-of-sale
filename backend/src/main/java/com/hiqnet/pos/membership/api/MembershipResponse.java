package com.hiqnet.pos.membership.api;

import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import java.time.Instant;
import java.util.UUID;

/** Administrative view of a branch membership (never exposes session data). */
public record MembershipResponse(
        UUID id,
        UUID organizationId,
        UUID branchId,
        UUID userId,
        BranchMembershipStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static MembershipResponse from(BranchMembership membership) {
        return new MembershipResponse(
                membership.getId(),
                membership.getOrganizationId(),
                membership.getBranchId(),
                membership.getUserId(),
                membership.getStatus(),
                membership.getVersion(),
                membership.getCreatedAt(),
                membership.getUpdatedAt());
    }
}