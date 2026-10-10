package com.hiqnet.pos.cashshift.domain;

import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;

public interface CashShiftRepository extends JpaRepository<CashShift, UUID>, JpaSpecificationExecutor<CashShift> {

    /**
     * Scoped detail lookup: resolves ONLY within the given organization, branch
     * AND register, so a shift can never be read across tenant, branch or
     * register (IDOR).
     */
    Optional<CashShift> findByIdAndOrganizationIdAndBranchIdAndCashRegisterId(
            UUID id, UUID organizationId, UUID branchId, UUID cashRegisterId);

    /** UX pre-check only; the partial unique index remains the final authority. */
    boolean existsByOrganizationIdAndBranchIdAndCashRegisterIdAndStatus(
            UUID organizationId, UUID branchId, UUID cashRegisterId, CashShiftStatus status);

    /**
     * The open shift of a register (if any). Kept for a future Sales module to
     * attach a sale to the current shift; no Sales code exists yet.
     */
    Optional<CashShift> findFirstByOrganizationIdAndBranchIdAndCashRegisterIdAndStatusOrderByOpenedAtDesc(
            UUID organizationId, UUID branchId, UUID cashRegisterId, CashShiftStatus status);
}
