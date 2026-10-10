package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;
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
 * Use cases for the organization-owned modifier group. Every lookup is scoped by
 * organization id. Mutations are recorded in the append-only audit log (same
 * transaction).
 */
@Service
public class ModifierGroupService {

    private final ModifierGroupRepository modifierGroupRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public ModifierGroupService(
            ModifierGroupRepository modifierGroupRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.modifierGroupRepository = modifierGroupRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public ModifierGroup create(
            UUID organizationId, String code, String name, int minSelections, int maxSelections) {
        Organization organization = requireOrganization(organizationId);
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null
                && modifierGroupRepository.existsByOrganizationIdAndCode(organizationId, normalizedCode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.MODIFIER_GROUP_CODE_ALREADY_EXISTS,
                    "A modifier group with this code already exists in the organization");
        }
        ModifierGroup group = ModifierGroup.create(organization, normalizedCode, name, minSelections, maxSelections);
        ModifierGroup saved = modifierGroupRepository.save(group);
        auditPort.record(AuditRecord.of(
                organizationId,
                "modifier.group.created",
                "modifier_group",
                saved.getId(),
                AuditMetadata.of("code", saved.getCode())));
        return saved;
    }

    @Transactional(readOnly = true)
    public ModifierGroup get(UUID organizationId, UUID groupId) {
        requireOrganization(organizationId);
        return modifierGroupRepository
                .findByIdAndOrganizationId(groupId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.MODIFIER_GROUP_NOT_FOUND, "Modifier group not found"));
    }

    @Transactional(readOnly = true)
    public Page<ModifierGroup> list(UUID organizationId, ModifierGroupStatus status, String q, Pageable pageable) {
        requireOrganization(organizationId);
        return modifierGroupRepository.findAll(buildSpecification(organizationId, status, q), pageable);
    }

    @Transactional
    public ModifierGroup update(UUID organizationId, UUID groupId, UpdateModifierGroupCommand command) {
        requireOrganization(organizationId);
        ModifierGroup group = modifierGroupRepository
                .findByIdAndOrganizationIdForUpdate(groupId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.MODIFIER_GROUP_NOT_FOUND, "Modifier group not found"));
        if (group.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        List<String> changedFields = new ArrayList<>();

        if (command.name() != null) {
            String before = group.getName();
            group.updateName(command.name());
            if (!group.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.minSelections() != null || command.maxSelections() != null) {
            int beforeMin = group.getMinSelections();
            int beforeMax = group.getMaxSelections();
            int newMin = command.minSelections() == null ? beforeMin : command.minSelections();
            int newMax = command.maxSelections() == null ? beforeMax : command.maxSelections();
            group.updateSelections(newMin, newMax);
            if (group.getMinSelections() != beforeMin || group.getMaxSelections() != beforeMax) {
                changedFields.add("selections");
            }
        }
        if (command.status() != null) {
            ModifierGroupStatus before = group.getStatus();
            group.updateStatus(command.status());
            if (group.getStatus() != before) {
                changedFields.add("status");
            }
        }

        ModifierGroup saved = modifierGroupRepository.saveAndFlush(group);
        if (!changedFields.isEmpty()) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("changedFields", changedFields);
            auditPort.record(AuditRecord.of(
                    organizationId, "modifier.group.updated", "modifier_group", saved.getId(),
                    AuditMetadata.of(metadata)));
        }
        return saved;
    }

    private static Specification<ModifierGroup> buildSpecification(
            UUID organizationId, ModifierGroupStatus status, String q) {
        return (root, query, cb) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(cb.equal(root.get("organization").get("id"), organizationId));
            if (status != null) {
                predicates.add(cb.equal(root.get("status"), status));
            }
            if (q != null && !q.isBlank()) {
                String trimmed = q.trim();
                predicates.add(cb.or(
                        cb.like(cb.lower(root.get("name")), "%" + trimmed.toLowerCase(Locale.ROOT) + "%"),
                        cb.like(cb.lower(root.get("code")), "%" + trimmed.toLowerCase(Locale.ROOT) + "%")));
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
}
