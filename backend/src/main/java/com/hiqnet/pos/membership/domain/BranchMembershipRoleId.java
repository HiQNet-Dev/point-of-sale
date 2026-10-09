package com.hiqnet.pos.membership.domain;

import java.io.Serializable;
import java.util.UUID;

/** Composite key of {@link BranchMembershipRole}. */
public record BranchMembershipRoleId(UUID organizationId, UUID membershipId, UUID roleId) implements Serializable {

    public BranchMembershipRoleId {
        if (organizationId == null || membershipId == null || roleId == null) {
            throw new IllegalArgumentException("organizationId, membershipId and roleId are required");
        }
    }
}