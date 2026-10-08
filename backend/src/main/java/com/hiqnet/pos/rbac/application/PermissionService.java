package com.hiqnet.pos.rbac.application;

import com.hiqnet.pos.rbac.domain.Permission;
import com.hiqnet.pos.rbac.domain.PermissionRepository;
import java.util.List;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/** Read-only access to the system permission catalog (seeded by Flyway). */
@Service
public class PermissionService {

    private final PermissionRepository permissionRepository;

    public PermissionService(PermissionRepository permissionRepository) {
        this.permissionRepository = permissionRepository;
    }

    @Transactional(readOnly = true)
    public List<Permission> list() {
        return permissionRepository.findAllByOrderByCodeAsc();
    }
}