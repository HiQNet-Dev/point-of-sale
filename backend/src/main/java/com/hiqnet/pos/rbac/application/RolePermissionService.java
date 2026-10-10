package com.hiqnet.pos.rbac.application;

import com.hiqnet.pos.rbac.domain.Permission;
import com.hiqnet.pos.rbac.domain.PermissionRepository;
import com.hiqnet.pos.rbac.domain.RolePermission;
import com.hiqnet.pos.rbac.domain.RolePermissionRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Manages the permission set of an organization role. The controller already
 * resolved the role scoped to the organization (404 otherwise) before calling
 * here. Replacement is atomic: validation happens BEFORE any mutation and the
 * delete+insert run in the same transaction, so an unknown permission leaves
 * the previous set untouched.
 */
@Service
public class RolePermissionService {

    private static final Logger log = LoggerFactory.getLogger(RolePermissionService.class);

    private final RolePermissionRepository rolePermissionRepository;
    private final PermissionRepository permissionRepository;
    private final AuditPort auditPort;

    public RolePermissionService(
            RolePermissionRepository rolePermissionRepository,
            PermissionRepository permissionRepository,
            AuditPort auditPort) {
        this.rolePermissionRepository = rolePermissionRepository;
        this.permissionRepository = permissionRepository;
        this.auditPort = auditPort;
    }

    @Transactional(readOnly = true)
    public List<Permission> list(UUID roleId) {
        List<String> codes = rolePermissionRepository.findByRoleId(roleId).stream()
                .map(RolePermission::getPermissionCode)
                .toList();
        return permissionRepository.findByCodeInOrderByCodeAsc(codes);
    }

    @Transactional
    public void replace(UUID organizationId, UUID roleId, List<String> permissionCodes) {
        List<String> normalized = permissionCodes.stream().map(String::trim).toList();
        if (new HashSet<>(normalized).size() != normalized.size()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "permissions must not contain duplicates");
        }
        // Validar TODO el catálogo ANTES de mutar (atomicidad): un permiso
        // desconocido deja el conjunto anterior intacto.
        Set<String> existing = new HashSet<>(
                permissionRepository.findByCodeInOrderByCodeAsc(normalized).stream()
                        .map(Permission::getCode)
                        .toList());
        for (String code : normalized) {
            if (!existing.contains(code)) {
                throw new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.PERMISSION_NOT_FOUND,
                        "Permission not found: " + code);
            }
        }
        rolePermissionRepository.deleteByRoleId(roleId);
        rolePermissionRepository.saveAll(normalized.stream()
                .map(code -> RolePermission.of(roleId, code))
                .toList());
        log.info(
                "security-event=role-permissions-updated roleId={} count={}",
                roleId,
                normalized.size());
        auditPort.record(AuditRecord.of(
                organizationId,
                "role.permissions.replaced",
                "role",
                roleId,
                AuditMetadata.of("permissionCount", normalized.size())));
    }
}