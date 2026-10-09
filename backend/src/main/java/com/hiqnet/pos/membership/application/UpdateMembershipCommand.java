package com.hiqnet.pos.membership.application;

import com.hiqnet.pos.membership.domain.BranchMembershipStatus;

/**
 * Administrative update of a {@code BranchMembership}. {@code expectedVersion}
 * is required (the version the client read); {@code status} is the only
 * editable field (no DELETE — inactivation is the way «down»).
 */
public record UpdateMembershipCommand(long expectedVersion, BranchMembershipStatus status) {}