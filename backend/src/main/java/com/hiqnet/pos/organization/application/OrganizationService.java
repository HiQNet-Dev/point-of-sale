package com.hiqnet.pos.organization.application;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.util.Locale;
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

    public OrganizationService(OrganizationRepository organizationRepository) {
        this.organizationRepository = organizationRepository;
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
        if (command.name() != null) {
            organization.updateName(command.name());
        }
        if (command.timezone() != null) {
            organization.updateTimezone(command.timezone());
        }
        if (command.currency() != null) {
            organization.updateCurrency(command.currency());
        }
        if (command.locale() != null) {
            organization.updateLocale(command.locale());
        }
        if (command.status() != null) {
            organization.updateStatus(command.status());
        }
        return organizationRepository.saveAndFlush(organization);
    }
}