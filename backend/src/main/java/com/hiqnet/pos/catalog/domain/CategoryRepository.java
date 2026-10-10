package com.hiqnet.pos.catalog.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface CategoryRepository extends JpaRepository<Category, UUID> {

    /** Scoped lookup: prevents accidental cross-organization access (IDOR). */
    Optional<Category> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /** Scoped locking lookup for administrative mutations (expectedVersion). */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select c from Category c where c.id = :id and c.organization.id = :organizationId")
    Optional<Category> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    Page<Category> findByOrganizationId(UUID organizationId, Pageable pageable);

    Page<Category> findByOrganizationIdAndStatus(
            UUID organizationId, CategoryStatus status, Pageable pageable);

    boolean existsByOrganizationIdAndCode(UUID organizationId, String code);
}
