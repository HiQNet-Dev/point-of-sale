package com.hiqnet.pos.rbac.application;

import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleRepository;
import com.hiqnet.pos.rbac.domain.UserOrganizationRole;
import com.hiqnet.pos.rbac.domain.UserOrganizationRoleRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.domain.UserRepository;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.UUID;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Organization-wide role assignments of a user. The controller already resolved
 * the user scoped to the organization (404 otherwise). Replacement is atomic
 * and cross-tenant safe: every requested role must belong to the same
 * organization or the whole operation rolls back.
 */
@Service
public class UserRoleService {

    private static final Logger log = LoggerFactory.getLogger(UserRoleService.class);

    private final UserOrganizationRoleRepository userOrganizationRoleRepository;
    private final RoleRepository roleRepository;
    private final UserRepository userRepository;
    private final AuditPort auditPort;

    public UserRoleService(
            UserOrganizationRoleRepository userOrganizationRoleRepository,
            RoleRepository roleRepository,
            UserRepository userRepository,
            AuditPort auditPort) {
        this.userOrganizationRoleRepository = userOrganizationRoleRepository;
        this.roleRepository = roleRepository;
        this.userRepository = userRepository;
        this.auditPort = auditPort;
    }

    @Transactional(readOnly = true)
    public List<Role> list(UUID organizationId, UUID userId) {
        requireUser(organizationId, userId);
        List<UUID> roleIds = userOrganizationRoleRepository
                .findByOrganizationIdAndUserId(organizationId, userId)
                .stream()
                .map(UserOrganizationRole::getRoleId)
                .toList();
        return roleRepository.findByIdInAndOrganizationId(roleIds, organizationId).stream()
                .sorted(Comparator.comparing(Role::getCode))
                .toList();
    }

    @Transactional
    public void replace(UUID organizationId, UUID userId, List<UUID> roleIds) {
        if (new HashSet<>(roleIds).size() != roleIds.size()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "roleIds must not contain duplicates");
        }
        // User must belong to the organization (IDOR-safe), checked before any mutation.
        requireUser(organizationId, userId);
        // All requested roles must exist in THIS organization; any cross-tenant
        // or unknown id fails the whole replace (404) before any mutation.
        List<Role> scopedRoles = roleRepository.findByIdInAndOrganizationId(roleIds, organizationId);
        if (scopedRoles.size() != new HashSet<>(roleIds).size()) {
            throw new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.ROLE_NOT_FOUND, "Role not found");
        }
        userOrganizationRoleRepository.deleteByOrganizationIdAndUserId(organizationId, userId);
        userOrganizationRoleRepository.saveAll(roleIds.stream()
                .map(roleId -> UserOrganizationRole.of(organizationId, userId, roleId))
                .toList());
        log.info(
                "security-event=user-roles-updated organizationId={} userId={} roles={}",
                organizationId,
                userId,
                roleIds.size());
        auditPort.record(AuditRecord.of(
                organizationId,
                "user.roles.replaced",
                "user",
                userId,
                AuditMetadata.of("roleCount", roleIds.size())));
    }

    private void requireUser(UUID organizationId, UUID userId) {
        if (userRepository.findByIdAndOrganizationId(userId, organizationId).isEmpty()) {
            throw new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.USER_NOT_FOUND, "User not found");
        }
    }
}