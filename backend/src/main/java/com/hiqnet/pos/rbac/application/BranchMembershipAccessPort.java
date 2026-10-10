package com.hiqnet.pos.rbac.application;

import java.util.UUID;

/**
 * Application port consumed by {@link AuthorizationService} to evaluate the
 * BRANCH-SPECIFIC leg of {@code hasBranchPermission(...)}.
 *
 * <p>The port is declared HERE (in the consuming package) and implemented in
 * the {@code membership} module, exactly like the user-session-revocation port
 * pattern: the compile direction stays {@code membership -> rbac}, so the
 * authorization service can consult membership data without creating a
 * {@code rbac -> membership} package cycle (the dependency injection container
 * wires the membership adapter at runtime).</p>
 */
public interface BranchMembershipAccessPort {

    /**
     * True when the user currently holds {@code permissionCode} for that exact
     * branch through an ACTIVE membership: user ACTIVE, organization ACTIVE,
     * branch ACTIVE and in the organization, membership ACTIVE, role ACTIVE,
     * in the same organization and assigned to the membership, role contains
     * the permission. PostgreSQL is the authority; no cache, no JWT claims.
     */
    boolean hasEffectiveBranchPermission(
            UUID userId, UUID organizationId, UUID branchId, String permissionCode);

    /**
     * RECOVERY variant of {@link #hasEffectiveBranchPermission}: the same
     * membership/role/user ACTIVE requirements, but the branch itself need NOT
     * be ACTIVE. Used only to authorize closing an already-open cash shift so a
     * deactivated branch cannot trap it behind a branch-scoped grant.
     */
    boolean hasBranchPermissionIgnoringBranchStatus(
            UUID userId, UUID organizationId, UUID branchId, String permissionCode);
}