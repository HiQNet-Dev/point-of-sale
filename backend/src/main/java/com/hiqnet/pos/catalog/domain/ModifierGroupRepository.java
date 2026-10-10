package com.hiqnet.pos.catalog.domain;

import jakarta.persistence.LockModeType;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface ModifierGroupRepository
        extends JpaRepository<ModifierGroup, UUID>, JpaSpecificationExecutor<ModifierGroup> {

    /** Scoped lookup: prevents accidental cross-organization access (IDOR). */
    Optional<ModifierGroup> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /** Scoped locking lookup for administrative mutations (expectedVersion). */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select g from ModifierGroup g where g.id = :id and g.organization.id = :organizationId")
    Optional<ModifierGroup> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    boolean existsByOrganizationIdAndCode(UUID organizationId, String code);

    /** Same-organization bulk lookup for validating product assignments. */
    List<ModifierGroup> findByOrganizationIdAndIdIn(UUID organizationId, Collection<UUID> ids);
}
