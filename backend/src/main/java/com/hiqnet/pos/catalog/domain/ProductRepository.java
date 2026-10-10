package com.hiqnet.pos.catalog.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface ProductRepository
        extends JpaRepository<Product, UUID>, JpaSpecificationExecutor<Product> {

    /** Scoped lookup: prevents accidental cross-organization access (IDOR). */
    Optional<Product> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /** Scoped locking lookup for administrative mutations (expectedVersion). */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select p from Product p where p.id = :id and p.organization.id = :organizationId")
    Optional<Product> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    boolean existsByOrganizationIdAndSku(UUID organizationId, String sku);

    boolean existsByOrganizationIdAndBarcode(UUID organizationId, String barcode);
}
