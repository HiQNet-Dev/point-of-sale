package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.ModifierOptionStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases for the organization-owned modifier option. Every lookup is scoped
 * by organization id and by the owning group. Mutations are recorded in the
 * append-only audit log (same transaction).
 */
@Service
public class ModifierOptionService {

    private final ModifierOptionRepository modifierOptionRepository;
    private final ModifierGroupRepository modifierGroupRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public ModifierOptionService(
            ModifierOptionRepository modifierOptionRepository,
            ModifierGroupRepository modifierGroupRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.modifierOptionRepository = modifierOptionRepository;
        this.modifierGroupRepository = modifierGroupRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public ModifierOption create(
            UUID organizationId,
            UUID groupId,
            String code,
            String name,
            BigDecimal priceDelta,
            int displayOrder) {
        Organization organization = requireOrganization(organizationId);
        ModifierGroup group = requireGroup(organizationId, groupId);
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null && modifierOptionRepository.existsByModifierGroupIdAndCode(groupId, normalizedCode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.MODIFIER_OPTION_CODE_ALREADY_EXISTS,
                    "A modifier option with this code already exists in the group");
        }
        ModifierOption option =
                ModifierOption.create(organization, group, normalizedCode, name, priceDelta, displayOrder);
        ModifierOption saved = modifierOptionRepository.save(option);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("code", saved.getCode());
        metadata.put("modifierGroupId", saved.getModifierGroup().getId().toString());
        auditPort.record(AuditRecord.of(
                organizationId,
                "modifier.option.created",
                "modifier_option",
                saved.getId(),
                AuditMetadata.of(metadata)));
        return saved;
    }

    @Transactional(readOnly = true)
    public ModifierOption get(UUID organizationId, UUID groupId, UUID optionId) {
        requireOrganization(organizationId);
        requireGroup(organizationId, groupId);
        return modifierOptionRepository
                .findByIdAndOrganizationId(optionId, organizationId)
                .filter(option -> option.getModifierGroup().getId().equals(groupId))
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.MODIFIER_OPTION_NOT_FOUND, "Modifier option not found"));
    }

    @Transactional(readOnly = true)
    public Page<ModifierOption> list(UUID organizationId, UUID groupId, Pageable pageable) {
        requireOrganization(organizationId);
        requireGroup(organizationId, groupId);
        return modifierOptionRepository.findByOrganizationIdAndModifierGroupId(organizationId, groupId, pageable);
    }

    @Transactional
    public ModifierOption update(
            UUID organizationId, UUID groupId, UUID optionId, UpdateModifierOptionCommand command) {
        requireOrganization(organizationId);
        requireGroup(organizationId, groupId);
        ModifierOption option = modifierOptionRepository
                .findByIdAndOrganizationIdForUpdate(optionId, organizationId)
                .filter(found -> found.getModifierGroup().getId().equals(groupId))
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.MODIFIER_OPTION_NOT_FOUND, "Modifier option not found"));
        if (option.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        List<String> changedFields = new ArrayList<>();

        if (command.name() != null) {
            String before = option.getName();
            option.updateName(command.name());
            if (!option.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.priceDelta() != null) {
            BigDecimal before = option.getPriceDelta();
            option.updatePriceDelta(command.priceDelta());
            if (option.getPriceDelta().compareTo(before) != 0) {
                changedFields.add("priceDelta");
            }
        }
        if (command.displayOrder() != null) {
            int before = option.getDisplayOrder();
            option.updateDisplayOrder(command.displayOrder());
            if (option.getDisplayOrder() != before) {
                changedFields.add("displayOrder");
            }
        }
        if (command.status() != null) {
            ModifierOptionStatus before = option.getStatus();
            option.updateStatus(command.status());
            if (option.getStatus() != before) {
                changedFields.add("status");
            }
        }

        ModifierOption saved = modifierOptionRepository.saveAndFlush(option);
        if (!changedFields.isEmpty()) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("changedFields", changedFields);
            auditPort.record(AuditRecord.of(
                    organizationId, "modifier.option.updated", "modifier_option", saved.getId(),
                    AuditMetadata.of(metadata)));
        }
        return saved;
    }

    private ModifierGroup requireGroup(UUID organizationId, UUID groupId) {
        return modifierGroupRepository
                .findByIdAndOrganizationId(groupId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.MODIFIER_GROUP_NOT_FOUND, "Modifier group not found"));
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
    }
}
