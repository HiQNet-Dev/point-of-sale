package com.hiqnet.pos.organization.application;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
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
 * Use cases for the organization aggregate. No HTTP knowledge here;
 * business rules and uniqueness live behind this boundary.
 */
@Service
public class OrganizationService {

    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public OrganizationService(OrganizationRepository organizationRepository, AuditPort auditPort) {
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public Organization create(String name, String code, String timezone, String currency, String locale) {
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null && organizationRepository.existsByCode(normalizedCode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.ORGANIZATION_CODE_ALREADY_EXISTS,
                    "An organization with this code already exists");
        }
        Organization organization = Organization.create(name, normalizedCode, timezone, currency, locale);
        return organizationRepository.save(organization);
    }

    @Transactional(readOnly = true)
    public Organization get(UUID organizationId) {
        return organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.ORGANIZATION_NOT_FOUND,
                        "Organization not found"));
    }

    @Transactional
    public Organization update(UUID organizationId, UpdateOrganizationCommand command) {
        Organization organization = get(organizationId);
        if (organization.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        List<String> changedFields = new ArrayList<>();
        OrganizationStatus statusBefore = organization.getStatus();
        if (command.name() != null) {
            String before = organization.getName();
            organization.updateName(command.name());
            if (!organization.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.timezone() != null) {
            String before = organization.getTimezone();
            organization.updateTimezone(command.timezone());
            if (!organization.getTimezone().equals(before)) {
                changedFields.add("timezone");
            }
        }
        if (command.currency() != null) {
            String before = organization.getCurrency();
            organization.updateCurrency(command.currency());
            if (!organization.getCurrency().equals(before)) {
                changedFields.add("currency");
            }
        }
        if (command.locale() != null) {
            String before = organization.getLocale();
            organization.updateLocale(command.locale());
            if (!organization.getLocale().equals(before)) {
                changedFields.add("locale");
            }
        }
        if (command.status() != null) {
            OrganizationStatus before = statusBefore;
            organization.updateStatus(command.status());
            if (organization.getStatus() != before) {
                changedFields.add("status");
            }
        }
        Organization saved = organizationRepository.saveAndFlush(organization);
        if (!changedFields.isEmpty()) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("changedFields", changedFields);
            if (changedFields.contains("status")) {
                metadata.put("fromStatus", statusBefore.name());
                metadata.put("toStatus", saved.getStatus().name());
            }
            auditPort.record(AuditRecord.of(
                    organizationId, "organization.updated", "organization", organizationId, AuditMetadata.of(metadata)));
        }
        return saved;
    }
}