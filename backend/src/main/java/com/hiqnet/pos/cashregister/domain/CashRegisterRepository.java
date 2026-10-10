package com.hiqnet.pos.cashregister.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface CashRegisterRepository
        extends JpaRepository<CashRegister, UUID>, JpaSpecificationExecutor<CashRegister> {

    /** Scoped lookup: prevents accidental cross-organization/cross-branch access (IDOR). */
    Optional<CashRegister> findByIdAndOrganizationIdAndBranchId(UUID id, UUID organizationId, UUID branchId);

    /** Scoped locking lookup for administrative mutations (expectedVersion). */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select r from CashRegister r where r.id = :id and r.organization.id = :organizationId "
            + "and r.branch.id = :branchId")
    Optional<CashRegister> findByIdAndOrganizationIdAndBranchIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId, @Param("branchId") UUID branchId);

    boolean existsByBranchIdAndCode(UUID branchId, String code);
}
