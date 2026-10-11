package com.hiqnet.pos.cashshift.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;

public interface CashShiftRepository extends JpaRepository<CashShift, UUID>, JpaSpecificationExecutor<CashShift> {

    /**
     * Scoped detail lookup: resolves ONLY within the given organization, branch
     * AND register, so a shift can never be read across tenant, branch or
     * register (IDOR).
     */
    Optional<CashShift> findByIdAndOrganizationIdAndBranchIdAndCashRegisterId(
            UUID id, UUID organizationId, UUID branchId, UUID cashRegisterId);

    /**
     * Coordination lock shared with sale creation. It prevents a close from
     * committing between an OPEN-sale check and an insert on the same shift.
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select s from CashShift s where s.id = :id and s.organizationId = :organizationId "
            + "and s.branchId = :branchId and s.cashRegisterId = :cashRegisterId")
    Optional<CashShift> findByIdAndOrganizationIdAndBranchIdAndCashRegisterIdForUpdate(
            @Param("id") UUID id,
            @Param("organizationId") UUID organizationId,
            @Param("branchId") UUID branchId,
            @Param("cashRegisterId") UUID cashRegisterId);

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
