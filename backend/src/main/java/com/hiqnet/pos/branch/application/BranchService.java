package com.hiqnet.pos.branch.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases for the branch aggregate. Every lookup is scoped by
 * organization id: a branch never resolves without its owner. Mutations are
 * recorded in the append-only audit log (same transaction).
 */
@Service
public class BranchService {

    private final BranchRepository branchRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public BranchService(
            BranchRepository branchRepository, OrganizationRepository organizationRepository, AuditPort auditPort) {
        this.branchRepository = branchRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public Branch create(UUID organizationId, String name, String code, String timezone) {
        Organization organization = organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.ORGANIZATION_NOT_FOUND,
                        "Organization not found"));
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null && branchRepository.existsByOrganizationIdAndCode(organizationId, normalizedCode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.BRANCH_CODE_ALREADY_EXISTS,
                    "A branch with this code already exists in the organization");
        }
        Branch branch = Branch.create(organization, name, normalizedCode, timezone);
        Branch saved = branchRepository.save(branch);
        auditPort.record(AuditRecord.ofBranch(
                organizationId,
                saved.getId(),
                "branch.created",
                "branch",
                saved.getId(),
                AuditMetadata.of("code", saved.getCode())));
        return saved;
    }

    @Transactional(readOnly = true)
    public List<Branch> list(UUID organizationId) {
        requireOrganization(organizationId);
        return branchRepository.findAllByOrganizationId(organizationId);
    }

    @Transactional(readOnly = true)
    public Branch get(UUID organizationId, UUID branchId) {
        requireOrganization(organizationId);
        return branchRepository
                .findByIdAndOrganizationId(branchId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found"));
    }

    @Transactional
    public Branch update(UUID organizationId, UUID branchId, UpdateBranchCommand command) {
        Branch branch = get(organizationId, branchId);
        if (branch.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        List<String> changedFields = new ArrayList<>();
        BranchStatus statusBefore = branch.getStatus();
        if (command.name() != null) {
            String before = branch.getName();
            branch.updateName(command.name());
            if (!branch.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.timezone() != null) {
            // PatchField null = omitted (no change); value may itself be null
            // to clear the override and inherit the organization timezone.
            String before = branch.getTimezone();
            branch.updateTimezone(command.timezone().value());
            if (!java.util.Objects.equals(branch.getTimezone(), before)) {
                changedFields.add("timezone");
            }
        }
        if (command.status() != null) {
            branch.updateStatus(command.status());
            if (branch.getStatus() != statusBefore) {
                changedFields.add("status");
            }
        }
        Branch saved = branchRepository.saveAndFlush(branch);
        if (!changedFields.isEmpty()) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("changedFields", changedFields);
            if (changedFields.contains("status")) {
                metadata.put("fromStatus", statusBefore.name());
                metadata.put("toStatus", saved.getStatus().name());
            }
            auditPort.record(AuditRecord.ofBranch(
                    organizationId,
                    saved.getId(),
                    "branch.updated",
                    "branch",
                    saved.getId(),
                    AuditMetadata.of(metadata)));
        }
        return saved;
    }

    private void requireOrganization(UUID organizationId) {
        if (!organizationRepository.existsById(organizationId)) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND,
                    ErrorCodes.ORGANIZATION_NOT_FOUND,
                    "Organization not found");
        }
    }
}