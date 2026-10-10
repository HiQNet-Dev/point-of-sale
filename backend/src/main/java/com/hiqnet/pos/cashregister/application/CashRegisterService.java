package com.hiqnet.pos.cashregister.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashregister.domain.CashRegisterStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.criteria.Predicate;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases for the branch-owned cash register. Every lookup is scoped by
 * organization AND branch id: a register never resolves without both. Mutations
 * are recorded in the append-only audit log (same transaction).
 */
@Service
public class CashRegisterService {

    private final CashRegisterRepository cashRegisterRepository;
    private final BranchRepository branchRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public CashRegisterService(
            CashRegisterRepository cashRegisterRepository,
            BranchRepository branchRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.cashRegisterRepository = cashRegisterRepository;
        this.branchRepository = branchRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public CashRegister create(UUID organizationId, UUID branchId, String code, String name) {
        Organization organization = requireOrganization(organizationId);
        Branch branch = requireBranch(organizationId, branchId);
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null && cashRegisterRepository.existsByBranchIdAndCode(branchId, normalizedCode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CASH_REGISTER_CODE_ALREADY_EXISTS,
                    "A cash register with this code already exists in the branch");
        }
        CashRegister register = CashRegister.create(organization, branch, normalizedCode, name);
        CashRegister saved = cashRegisterRepository.save(register);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("branchId", saved.getBranch().getId().toString());
        metadata.put("code", saved.getCode());
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "cash-register.created", "cash_register", saved.getId(),
                AuditMetadata.of(metadata)));
        return saved;
    }

    @Transactional(readOnly = true)
    public CashRegister get(UUID organizationId, UUID branchId, UUID cashRegisterId) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        return cashRegisterRepository
                .findByIdAndOrganizationIdAndBranchId(cashRegisterId, organizationId, branchId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CASH_REGISTER_NOT_FOUND, "Cash register not found"));
    }

    @Transactional(readOnly = true)
    public Page<CashRegister> list(UUID organizationId, UUID branchId, CashRegisterStatus status, String q,
            Pageable pageable) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        return cashRegisterRepository.findAll(buildSpecification(organizationId, branchId, status, q), pageable);
    }

    @Transactional
    public CashRegister update(UUID organizationId, UUID branchId, UUID cashRegisterId,
            UpdateCashRegisterCommand command) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        CashRegister register = cashRegisterRepository
                .findByIdAndOrganizationIdAndBranchIdForUpdate(cashRegisterId, organizationId, branchId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CASH_REGISTER_NOT_FOUND, "Cash register not found"));
        if (register.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        List<String> changedFields = new ArrayList<>();

        if (command.name() != null) {
            String before = register.getName();
            register.updateName(command.name());
            if (!register.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.status() != null) {
            CashRegisterStatus before = register.getStatus();
            register.updateStatus(command.status());
            if (register.getStatus() != before) {
                changedFields.add("status");
            }
        }

        CashRegister saved = cashRegisterRepository.saveAndFlush(register);
        if (!changedFields.isEmpty()) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("changedFields", changedFields);
            auditPort.record(AuditRecord.ofBranch(
                    organizationId, branchId, "cash-register.updated", "cash_register", saved.getId(),
                    AuditMetadata.of(metadata)));
        }
        return saved;
    }

    private static Specification<CashRegister> buildSpecification(
            UUID organizationId, UUID branchId, CashRegisterStatus status, String q) {
        return (root, query, cb) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(cb.equal(root.get("organization").get("id"), organizationId));
            predicates.add(cb.equal(root.get("branch").get("id"), branchId));
            if (status != null) {
                predicates.add(cb.equal(root.get("status"), status));
            }
            if (q != null && !q.isBlank()) {
                String trimmed = q.trim().toLowerCase(Locale.ROOT);
                predicates.add(cb.or(
                        cb.like(cb.lower(root.get("name")), "%" + trimmed + "%"),
                        cb.like(cb.lower(root.get("code")), "%" + trimmed + "%")));
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
}
