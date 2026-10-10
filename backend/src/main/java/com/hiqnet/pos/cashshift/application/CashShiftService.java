package com.hiqnet.pos.cashshift.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.cashshift.domain.CashShiftStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.criteria.Predicate;
import java.math.BigDecimal;
import java.time.Clock;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases for branch/register-owned cash shifts. Every lookup is scoped by
 * organization AND branch AND register id. The "at most one OPEN shift" rule is
 * enforced by the database partial unique index, not by this service: the
 * {@code existsOpenShift} pre-check is UX-only and a concurrent double-open is
 * resolved by PostgreSQL (mapped to 409 by {@code ConstraintTranslator}).
 */
@Service
public class CashShiftService {

    private final CashShiftRepository cashShiftRepository;
    private final CashRegisterRepository cashRegisterRepository;
    private final BranchRepository branchRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;
    private final Clock clock;

    public CashShiftService(
            CashShiftRepository cashShiftRepository,
            CashRegisterRepository cashRegisterRepository,
            BranchRepository branchRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort,
            Clock clock) {
        this.cashShiftRepository = cashShiftRepository;
        this.cashRegisterRepository = cashRegisterRepository;
        this.branchRepository = branchRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
        this.clock = clock;
    }

    @Transactional
    public CashShift open(
            UUID organizationId,
            UUID branchId,
            UUID cashRegisterId,
            UUID actorUserId,
            BigDecimal openingAmount) {
        requireOrganization(organizationId);
        Branch branch = requireBranch(organizationId, branchId);
        CashRegister register = requireCashRegister(organizationId, branchId, cashRegisterId);
        if (branch.getStatus() != BranchStatus.ACTIVE) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.BRANCH_INACTIVE, "The branch is inactive and cannot open a cash shift");
        }
        if (!register.isActive()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CASH_REGISTER_INACTIVE,
                    "The cash register is inactive and cannot open a cash shift");
        }
        // UX pre-check only: the partial unique index is the final authority, so
        // a concurrent open is still rejected even if this read passes.
        if (cashShiftRepository.existsByOrganizationIdAndBranchIdAndCashRegisterIdAndStatus(
                organizationId, branchId, cashRegisterId, CashShiftStatus.OPEN)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT,
                    "The cash register already has an open cash shift");
        }
        CashShift shift =
                CashShift.open(organizationId, branchId, cashRegisterId, actorUserId, openingAmount, clock.instant());
        CashShift saved = cashShiftRepository.saveAndFlush(shift);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("branchId", branchId.toString());
        metadata.put("cashRegisterId", cashRegisterId.toString());
        metadata.put("openingAmount", saved.getOpeningAmount().toPlainString());
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "cash-shift.opened", "cash_shift", saved.getId(), AuditMetadata.of(metadata)));
        return saved;
    }

    @Transactional
    public CashShift close(
            UUID organizationId,
            UUID branchId,
            UUID cashRegisterId,
            UUID cashShiftId,
            long expectedVersion,
            BigDecimal declaredCashAmount,
            UUID actorUserId) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        // Deliberately NO active-status check here: an already OPEN shift must
        // remain closable even if the register (or branch) was deactivated after
        // it opened. The ACTIVE requirement applies to opening only.
        requireCashRegister(organizationId, branchId, cashRegisterId);
        CashShift shift = cashShiftRepository
                .findByIdAndOrganizationIdAndBranchIdAndCashRegisterId(cashShiftId, organizationId, branchId, cashRegisterId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CASH_SHIFT_NOT_FOUND, "Cash shift not found"));
        if (shift.getStatus() == CashShiftStatus.CLOSED) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.CASH_SHIFT_ALREADY_CLOSED, "The cash shift is already closed");
        }
        // Explicit expectedVersion guard rejects a stale write with 409; @Version
        // on the row makes the DB update predicate the final authority under a
        // real race, so exactly one concurrent close can win.
        if (shift.getVersion() != expectedVersion) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        shift.close(declaredCashAmount, actorUserId, clock.instant());
        CashShift saved = cashShiftRepository.saveAndFlush(shift);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("cashRegisterId", cashRegisterId.toString());
        metadata.put("declaredCashAmount", saved.getClosingDeclaredAmount().toPlainString());
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "cash-shift.closed", "cash_shift", saved.getId(), AuditMetadata.of(metadata)));
        return saved;
    }

    @Transactional(readOnly = true)
    public CashShift get(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        requireCashRegister(organizationId, branchId, cashRegisterId);
        return cashShiftRepository
                .findByIdAndOrganizationIdAndBranchIdAndCashRegisterId(cashShiftId, organizationId, branchId, cashRegisterId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CASH_SHIFT_NOT_FOUND, "Cash shift not found"));
    }

    @Transactional(readOnly = true)
    public Page<CashShift> list(
            UUID organizationId, UUID branchId, UUID cashRegisterId, CashShiftStatus status, Pageable pageable) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        requireCashRegister(organizationId, branchId, cashRegisterId);
        return cashShiftRepository.findAll(
                buildSpecification(organizationId, branchId, cashRegisterId, status), pageable);
    }

    /**
     * The current OPEN shift of a register, if any. Kept for a future Sales
     * module; there is no Sales coupling now.
     */
    @Transactional(readOnly = true)
    public Optional<CashShift> findOpen(UUID organizationId, UUID branchId, UUID cashRegisterId) {
        return cashShiftRepository.findFirstByOrganizationIdAndBranchIdAndCashRegisterIdAndStatusOrderByOpenedAtDesc(
                organizationId, branchId, cashRegisterId, CashShiftStatus.OPEN);
    }

    private static Specification<CashShift> buildSpecification(
            UUID organizationId, UUID branchId, UUID cashRegisterId, CashShiftStatus status) {
        return (root, query, cb) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(cb.equal(root.get("organizationId"), organizationId));
            predicates.add(cb.equal(root.get("branchId"), branchId));
            predicates.add(cb.equal(root.get("cashRegisterId"), cashRegisterId));
            if (status != null) {
                predicates.add(cb.equal(root.get("status"), status));
            }
            return cb.and(predicates.toArray(new Predicate[0]));
        };
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
    }

    private Branch requireBranch(UUID organizationId, UUID branchId) {
        return branchRepository
                .findByIdAndOrganizationId(branchId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found"));
    }

    private CashRegister requireCashRegister(UUID organizationId, UUID branchId, UUID cashRegisterId) {
        return cashRegisterRepository
                .findByIdAndOrganizationIdAndBranchId(cashRegisterId, organizationId, branchId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CASH_REGISTER_NOT_FOUND, "Cash register not found"));
    }
}
