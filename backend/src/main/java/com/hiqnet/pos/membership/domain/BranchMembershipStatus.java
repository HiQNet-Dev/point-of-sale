package com.hiqnet.pos.membership.domain;

/**
 * Lifecycle of a {@link BranchMembership}. INACTIVE memberships keep their
 * configuration (branch link, role assignments) but stop granting any branch
 * permission; reactivation makes the previously assigned roles effective again.
 */
public enum BranchMembershipStatus {
    ACTIVE,
    INACTIVE
}