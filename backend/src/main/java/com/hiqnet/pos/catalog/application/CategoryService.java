package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryRepository;
import com.hiqnet.pos.catalog.domain.CategoryStatus;
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
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases for the organization-owned category. Every lookup is scoped by
 * organization id (a category never resolves without its owner). Mutations are
 * recorded in the append-only audit log (same transaction).
 */
@Service
public class CategoryService {

    private final CategoryRepository categoryRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public CategoryService(
            CategoryRepository categoryRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.categoryRepository = categoryRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public Category create(UUID organizationId, String code, String name) {
        Organization organization = requireOrganization(organizationId);
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null
                && categoryRepository.existsByOrganizationIdAndCode(organizationId, normalizedCode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CATEGORY_CODE_ALREADY_EXISTS,
                    "A category with this code already exists in the organization");
        }
        Category category = Category.create(organization, normalizedCode, name);
        Category saved = categoryRepository.save(category);
        auditPort.record(AuditRecord.of(
                organizationId,
                "category.created",
                "category",
                saved.getId(),
                AuditMetadata.of("code", saved.getCode())));
        return saved;
    }

    @Transactional(readOnly = true)
    public Category get(UUID organizationId, UUID categoryId) {
        requireOrganization(organizationId);
        return categoryRepository
                .findByIdAndOrganizationId(categoryId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CATEGORY_NOT_FOUND, "Category not found"));
    }

    @Transactional(readOnly = true)
    public Page<Category> list(UUID organizationId, CategoryStatus status, Pageable pageable) {
        requireOrganization(organizationId);
        if (status != null) {
            return categoryRepository.findByOrganizationIdAndStatus(organizationId, status, pageable);
        }
        return categoryRepository.findByOrganizationId(organizationId, pageable);
    }

    @Transactional
    public Category update(UUID organizationId, UUID categoryId, UpdateCategoryCommand command) {
        requireOrganization(organizationId);
        Category category = categoryRepository
                .findByIdAndOrganizationIdForUpdate(categoryId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CATEGORY_NOT_FOUND, "Category not found"));
        if (category.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        List<String> changedFields = new ArrayList<>();
        CategoryStatus statusBefore = category.getStatus();
        if (command.name() != null) {
            String before = category.getName();
            category.updateName(command.name());
            if (!category.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.status() != null) {
            category.updateStatus(command.status());
            if (category.getStatus() != statusBefore) {
                changedFields.add("status");
            }
        }
        Category saved = categoryRepository.saveAndFlush(category);
        if (!changedFields.isEmpty()) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("changedFields", changedFields);
            auditPort.record(AuditRecord.of(
                    organizationId, "category.updated", "category", saved.getId(), AuditMetadata.of(metadata)));
        }
        return saved;
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
    }
}
