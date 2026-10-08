package com.hiqnet.pos.rbac.application;

import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleRepository;
import com.hiqnet.pos.rbac.domain.RoleStatus;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.util.UUID;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Organizational role use cases. Roles are organization-scoped, code is a
 * normalized immutable slug (never derived from name), and there is no physical
 * delete: deactivation is {@code status = INACTIVE} (grants nothing while kept).
 */
@Service
public class RoleService {

    private static final Logger log = LoggerFactory.getLogger(RoleService.class);

    private final RoleRepository roleRepository;
    private final OrganizationRepository organizationRepository;

    public RoleService(RoleRepository roleRepository, OrganizationRepository organizationRepository) {
        this.roleRepository = roleRepository;
        this.organizationRepository = organizationRepository;
    }

    @Transactional
    public Role create(UUID organizationId, String code, String name) {
        requireOrganization(organizationId);
        String normalizedCode = Role.normalizeCode(code);
        if (roleRepository.existsByOrganizationIdAndCode(organizationId, normalizedCode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.ROLE_CODE_ALREADY_EXISTS,
                    "A role with this code already exists in the organization");
        }
        Role role = Role.create(organizationId, normalizedCode, name);
        Role saved = roleRepository.save(role);
        log.info(
                "security-event=role-created organizationId={} roleId={} code={}",
                organizationId,
                saved.getId(),
                saved.getCode());
        return saved;
    }

    @Transactional(readOnly = true)
    public Role get(UUID organizationId, UUID roleId) {
        return requireRole(organizationId, roleId);
    }

    @Transactional(readOnly = true)
    public Page<Role> list(UUID organizationId, RoleStatus status, Pageable pageable) {
        requireOrganization(organizationId);
        return status == null
                ? roleRepository.findByOrganizationId(organizationId, pageable)
                : roleRepository.findByOrganizationIdAndStatus(organizationId, status, pageable);
    }

    @Transactional
    public Role update(UUID organizationId, UUID roleId, UpdateRoleCommand command) {
        Role role = requireRole(organizationId, roleId);
        if (role.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        if (command.name() != null) {
            role.updateName(command.name());
        }
        if (command.status() != null) {
            role.updateStatus(command.status());
        }
        Role updated = roleRepository.saveAndFlush(role);
        log.info(
                "security-event=role-updated organizationId={} roleId={} code={}",
                organizationId,
                updated.getId(),
                updated.getCode());
        return updated;
    }

    private Role requireRole(UUID organizationId, UUID roleId) {
        return roleRepository
                .findByIdAndOrganizationId(roleId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ROLE_NOT_FOUND, "Role not found"));
    }

    private void requireOrganization(UUID organizationId) {
        if (!organizationRepository.existsById(organizationId)) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found");
        }
    }
}