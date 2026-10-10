package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductModifierGroup;
import com.hiqnet.pos.catalog.domain.ProductModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.time.Instant;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.function.Function;
import java.util.stream.Collectors;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases for the ordered modifier-group configuration of a product. The
 * product is the concurrency boundary: a real replacement bumps the product's
 * version (touching {@code updatedAt}), while an identical (no-op) list changes
 * nothing and writes no audit event. Every lookup is scoped by organization id.
 */
@Service
public class ProductModifierService {

    private final ProductRepository productRepository;
    private final ModifierGroupRepository modifierGroupRepository;
    private final ProductModifierGroupRepository productModifierGroupRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public ProductModifierService(
            ProductRepository productRepository,
            ModifierGroupRepository modifierGroupRepository,
            ProductModifierGroupRepository productModifierGroupRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.productRepository = productRepository;
        this.modifierGroupRepository = modifierGroupRepository;
        this.productModifierGroupRepository = productModifierGroupRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional(readOnly = true)
    public List<ProductModifierAssignment> list(UUID organizationId, UUID productId) {
        requireOrganization(organizationId);
        requireProduct(organizationId, productId);
        List<ProductModifierGroup> rows =
                productModifierGroupRepository.findByProductIdAndOrganizationIdOrderByDisplayOrderAsc(
                        productId, organizationId);
        return toAssignments(organizationId, rows);
    }

    /**
     * Replaces the product's modifier-group configuration with the given ordered
     * list. Position in the list becomes {@code display_order}. Duplicate group
     * ids are rejected with 400; an unknown or cross-organization group id is
     * 404. An identical list is a no-op (no version bump, no audit event).
     */
    @Transactional
    public List<ProductModifierAssignment> replace(
            UUID organizationId, UUID productId, long expectedVersion, List<UUID> groupIds) {
        requireOrganization(organizationId);
        Product product = productRepository
                .findByIdAndOrganizationIdForUpdate(productId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.PRODUCT_NOT_FOUND, "Product not found"));
        if (product.getVersion() != expectedVersion) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }

        List<UUID> requested = groupIds == null ? List.of() : List.copyOf(groupIds);
        if (new HashSet<>(requested).size() != requested.size()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "modifierGroupIds must not contain duplicates");
        }
        Map<UUID, ModifierGroup> groupsById = resolveGroups(organizationId, requested);

        List<ProductModifierGroup> existing =
                productModifierGroupRepository.findByProductIdAndOrganizationIdOrderByDisplayOrderAsc(
                        productId, organizationId);
        List<UUID> existingIds = existing.stream()
                .map(ProductModifierGroup::getModifierGroupId)
                .toList();
        if (existingIds.equals(requested)) {
            // Identical configuration: no mutation, no version bump, no audit.
            return toAssignments(organizationId, existing);
        }

        productModifierGroupRepository.deleteAll(existing);
        productModifierGroupRepository.flush();

        List<ProductModifierGroup> rows = new ArrayList<>(requested.size());
        for (int index = 0; index < requested.size(); index++) {
            rows.add(ProductModifierGroup.of(organizationId, productId, requested.get(index), index));
        }
        productModifierGroupRepository.saveAll(rows);

        product.markConfigurationChanged(Instant.now());
        productRepository.saveAndFlush(product);

        auditPort.record(AuditRecord.of(
                organizationId,
                "product.modifier-groups.replaced",
                "product",
                productId,
                AuditMetadata.of("modifierGroupCount", requested.size())));

        return toAssignments(organizationId, rows);
    }

    private Map<UUID, ModifierGroup> resolveGroups(UUID organizationId, List<UUID> requested) {
        if (requested.isEmpty()) {
            return Map.of();
        }
        List<ModifierGroup> found = modifierGroupRepository.findByOrganizationIdAndIdIn(organizationId, requested);
        if (found.size() != requested.size()) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND,
                    ErrorCodes.MODIFIER_GROUP_NOT_FOUND,
                    "One or more modifier groups were not found in the organization");
        }
        return found.stream().collect(Collectors.toMap(ModifierGroup::getId, Function.identity()));
    }

    private List<ProductModifierAssignment> toAssignments(
            UUID organizationId, List<ProductModifierGroup> rows) {
        if (rows.isEmpty()) {
            return List.of();
        }
        List<UUID> ids = rows.stream().map(ProductModifierGroup::getModifierGroupId).toList();
        Map<UUID, ModifierGroup> groupsById = resolveGroups(organizationId, ids);
        return rows.stream()
                .map(row -> new ProductModifierAssignment(
                        groupsById.get(row.getModifierGroupId()), row.getDisplayOrder()))
                .toList();
    }

    private void requireProduct(UUID organizationId, UUID productId) {
        productRepository
                .findByIdAndOrganizationId(productId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.PRODUCT_NOT_FOUND, "Product not found"));
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
    }
}
