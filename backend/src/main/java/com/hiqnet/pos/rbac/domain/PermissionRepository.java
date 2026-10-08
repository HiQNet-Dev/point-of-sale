package com.hiqnet.pos.rbac.domain;

import java.util.Collection;
import java.util.List;
import org.springframework.data.jpa.repository.JpaRepository;

public interface PermissionRepository extends JpaRepository<Permission, String> {

    List<Permission> findAllByOrderByCodeAsc();

    List<Permission> findByCodeInOrderByCodeAsc(Collection<String> codes);
}