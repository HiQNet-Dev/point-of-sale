package com.hiqnet.pos.rbac.domain;

import jakarta.persistence.LockModeType;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface RoleRepository extends JpaRepository<Role, UUID> {

    /** Scoped lookup: a role is never resolved by a global id (IDOR). */
    Optional<Role> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /** Scoped locking lookup for administrative mutations (expectedVersion). */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select r from Role r where r.id = :id and r.organizationId = :organizationId")
    Optional<Role> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    Page<Role> findByOrganizationId(UUID organizationId, Pageable pageable);

    Page<Role> findByOrganizationIdAndStatus(UUID organizationId, RoleStatus status, Pageable pageable);

    boolean existsByOrganizationIdAndCode(UUID organizationId, String code);

    List<Role> findByIdInAndOrganizationId(Collection<UUID> ids, UUID organizationId);
}