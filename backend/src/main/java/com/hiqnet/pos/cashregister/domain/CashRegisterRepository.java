package com.hiqnet.pos.cashregister.domain;

import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;

public interface CashRegisterRepository
        extends JpaRepository<CashRegister, UUID>, JpaSpecificationExecutor<CashRegister> {

    /** Scoped lookup: prevents accidental cross-organization/cross-branch access (IDOR). */
    Optional<CashRegister> findByIdAndOrganizationIdAndBranchId(UUID id, UUID organizationId, UUID branchId);

    boolean existsByBranchIdAndCode(UUID branchId, String code);
}
