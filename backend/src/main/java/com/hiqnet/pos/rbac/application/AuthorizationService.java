package com.hiqnet.pos.rbac.application;

import com.hiqnet.pos.rbac.domain.UserOrganizationRoleRepository;
import java.util.UUID;
import org.springframework.security.core.Authentication;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Central authorization gate used by method security
 * ({@code @PreAuthorize("@authorizationService.hasPermission(...)")}).
 *
 * <p>JWT is identity-only: roles and permissions NEVER travel in the token.
 * PostgreSQL is the authority for authorization — every evaluation runs an
 * {@code EXISTS} query that checks, in one shot, that user and organization are
 * ACTIVE, the user belongs to the organization, the role is ACTIVE, assigned to
 * the user, and contains the requested permission. No cache, no
 * securityVersion, no N+1.</p>
 */
@Service
public class AuthorizationService {

    private final UserOrganizationRoleRepository userOrganizationRoleRepository;

    public AuthorizationService(UserOrganizationRoleRepository userOrganizationRoleRepository) {
        this.userOrganizationRoleRepository = userOrganizationRoleRepository;
    }

    @Transactional(readOnly = true)
    public boolean hasPermission(Authentication authentication, UUID organizationId, String permissionCode) {
        if (authentication == null || !(authentication.getPrincipal() instanceof Jwt jwt)) {
            return false;
        }
        UUID userId = parseUserId(jwt.getSubject());
        if (userId == null) {
            return false;
        }
        return userOrganizationRoleRepository.existsEffectivePermission(userId, organizationId, permissionCode);
    }

    private static UUID parseUserId(String subject) {
        try {
            return UUID.fromString(subject);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }
}