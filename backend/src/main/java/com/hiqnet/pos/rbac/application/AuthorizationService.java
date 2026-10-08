package com.hiqnet.pos.rbac.application;

import com.hiqnet.pos.rbac.domain.UserOrganizationRoleRepository;
import java.util.UUID;
import org.springframework.security.core.Authentication;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Central authorization gate used by method security
 * ({@code @PreAuthorize("@authorizationService.hasPermission(...)")} and …
 * {@code hasBranchPermission(...)}).
 *
 * <p>JWT is identity-only: roles and permissions NEVER travel in the token.
 * PostgreSQL is the authority for authorization. Two explicit scopes coexist:</p>
 *
 * <ul>
 *   <li><b>ORGANIZATION scope</b> — {@code hasPermission(...)}: the user holds
 *       the permission org-wide (user_organization_roles).</li>
 *   <li><b>BRANCH scope</b> — {@code hasBranchPermission(...)}: grants when the
 *       user holds the permission org-wide (organization override) OR through an
 *       ACTIVE branch membership of that exact branch. It NEVER promotes
 *       branch-scoped roles to organization scope: organization-wide endpoints
 *       only evaluate the organization check.</li>
 * </ul>
 *
 * <p>No cache, no securityVersion, no N+1: organization checks run a single
 * {@code EXISTS} query (rbac) and branch checks add the branch-scoped
 * {@code EXISTS} through {@link BranchMembershipAccessPort} (implemented in the
 * membership module; no {@code rbac -> membership} package cycle).</p>
 */
@Service
public class AuthorizationService {

    private final UserOrganizationRoleRepository userOrganizationRoleRepository;
    private final BranchMembershipAccessPort branchMembershipAccessPort;

    public AuthorizationService(
            UserOrganizationRoleRepository userOrganizationRoleRepository,
            BranchMembershipAccessPort branchMembershipAccessPort) {
        this.userOrganizationRoleRepository = userOrganizationRoleRepository;
        this.branchMembershipAccessPort = branchMembershipAccessPort;
    }

    /**
     * ORGANIZATION scope. True when the user currently holds the permission for
     * the whole organization: user ACTIVE, organization ACTIVE, user belongs to
     * the organization, role ACTIVE, assigned to the user, role contains the
     * permission.
     */
    @Transactional(readOnly = true)
    public boolean hasPermission(Authentication authentication, UUID organizationId, String permissionCode) {
        UUID userId = identityId(authentication);
        if (userId == null) {
            return false;
        }
        return userOrganizationRoleRepository.existsEffectivePermission(userId, organizationId, permissionCode);
    }

    /**
     * BRANCH scope. True when the user holds the permission for THAT branch:
     * either through the organization scope (override — the service layer still
     * isolates a foreign or missing branch id with 404) or through an ACTIVE
     * branch membership of the exact branch (branch ACTIVE included in the
     * query). Existing organization-wide grants are preserved unchanged.
     */
    @Transactional(readOnly = true)
    public boolean hasBranchPermission(
            Authentication authentication, UUID organizationId, UUID branchId, String permissionCode) {
        UUID userId = identityId(authentication);
        if (userId == null) {
            return false;
        }
        // Organization override: an org-wide authority grants branch access
        // regardless of the specific branch. A branch id that does not exist in
        // this organization falls through to the service, which answers 404
        // (established cross-org resource-isolation contract).
        if (userOrganizationRoleRepository.existsEffectivePermission(userId, organizationId, permissionCode)) {
            return true;
        }
        // Branch-specific leg: an ACTIVE membership grants the permission ONLY
        // for this branch; the query requires branch ACTIVE and same-org.
        return branchMembershipAccessPort.hasEffectiveBranchPermission(userId, organizationId, branchId, permissionCode);
    }

    private static UUID identityId(Authentication authentication) {
        if (authentication == null || !(authentication.getPrincipal() instanceof Jwt jwt)) {
            return null;
        }
        try {
            return UUID.fromString(jwt.getSubject());
        } catch (IllegalArgumentException e) {
            return null;
        }
    }
}