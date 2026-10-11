package com.hiqnet.pos.sales.domain;

import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;

public interface SaleRepository extends JpaRepository<Sale, UUID> {
    Optional<Sale> findByIdAndOrganizationIdAndBranchIdAndCashRegisterIdAndCashShiftId(
            UUID id, UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId);

    Page<Sale> findByOrganizationIdAndBranchIdAndCashRegisterIdAndCashShiftIdAndStatus(
            UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, SaleStatus status, Pageable pageable);

    Page<Sale> findByOrganizationIdAndBranchIdAndCashRegisterIdAndCashShiftId(
            UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, Pageable pageable);

    boolean existsByCashShiftIdAndStatus(UUID cashShiftId, SaleStatus status);
}
