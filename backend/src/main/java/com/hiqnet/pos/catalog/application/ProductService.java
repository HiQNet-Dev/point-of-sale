package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.catalog.domain.ProductStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.api.UuidPatchField;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.criteria.Predicate;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases for the organization-owned product. Every lookup is scoped by
 * organization id. Category references are validated inside the same
 * organization and must be ACTIVE; a category from another organization is
 * treated as NOT_FOUND (no existence leak). Mutations are recorded in the
 * append-only audit log (same transaction).
 */
@Service
public class ProductService {

    private final ProductRepository productRepository;
    private final CategoryRepository categoryRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public ProductService(
            ProductRepository productRepository,
            CategoryRepository categoryRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.productRepository = productRepository;
        this.categoryRepository = categoryRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public Product create(
            UUID organizationId,
            String sku,
            String barcode,
            String name,
            String description,
            UUID categoryId,
            BigDecimal unitPrice) {
        Organization organization = requireOrganization(organizationId);
        String normalizedSku = sku.trim().toUpperCase(Locale.ROOT);
        if (productRepository.existsByOrganizationIdAndSku(organizationId, normalizedSku)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PRODUCT_SKU_ALREADY_EXISTS,
                    "A product with this sku already exists in the organization");
        }
        String normalizedBarcode = normalizeOptional(barcode);
        if (normalizedBarcode != null
                && productRepository.existsByOrganizationIdAndBarcode(organizationId, normalizedBarcode)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PRODUCT_BARCODE_ALREADY_EXISTS,
                    "A product with this barcode already exists in the organization");
        }
        Category category = categoryId == null ? null : resolveActiveCategory(organizationId, categoryId);
        Product product = Product.create(organization, normalizedSku, name, unitPrice);
        product.updateBarcode(normalizedBarcode);
        product.updateDescription(description);
        if (category != null) {
            product.assignCategory(category);
        }
        Product saved = productRepository.save(product);
        auditPort.record(AuditRecord.of(
                organizationId,
                "product.created",
                "product",
                saved.getId(),
                AuditMetadata.of("sku", saved.getSku())));
        return saved;
    }

    @Transactional(readOnly = true)
    public Product get(UUID organizationId, UUID productId) {
        requireOrganization(organizationId);
        return productRepository
                .findByIdAndOrganizationId(productId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.PRODUCT_NOT_FOUND, "Product not found"));
    }

    @Transactional(readOnly = true)
    public Page<Product> list(
            UUID organizationId, ProductStatus status, UUID categoryId, String q, Pageable pageable) {
        requireOrganization(organizationId);
        if (categoryId != null) {
            // A category filter from another organization surfaces as 404, not
            // as an empty page (no existence leak across organizations).
            categoryRepository
                    .findByIdAndOrganizationId(categoryId, organizationId)
                    .orElseThrow(() -> new BusinessException(
                            HttpStatus.NOT_FOUND, ErrorCodes.CATEGORY_NOT_FOUND, "Category not found"));
        }
        return productRepository.findAll(buildSpecification(organizationId, status, categoryId, q), pageable);
    }

    @Transactional
    public Product update(UUID organizationId, UUID productId, UpdateProductCommand command) {
        requireOrganization(organizationId);
        Product product = productRepository
                .findByIdAndOrganizationIdForUpdate(productId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.PRODUCT_NOT_FOUND, "Product not found"));
        if (product.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        List<String> changedFields = new ArrayList<>();

        if (command.name() != null) {
            String before = product.getName();
            product.updateName(command.name());
            if (!product.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.description() != null) {
            String before = product.getDescription();
            product.updateDescription(command.description());
            if (!Objects.equals(product.getDescription(), before)) {
                changedFields.add("description");
            }
        }
        if (command.barcode() != null) {
            String normalizedBarcode = normalizeOptional(command.barcode());
            if (normalizedBarcode != null
                    && !normalizedBarcode.equals(product.getBarcode())
                    && productRepository.existsByOrganizationIdAndBarcode(organizationId, normalizedBarcode)) {
                throw new BusinessException(
                        HttpStatus.CONFLICT,
                        ErrorCodes.PRODUCT_BARCODE_ALREADY_EXISTS,
                        "A product with this barcode already exists in the organization");
            }
            String before = product.getBarcode();
            product.updateBarcode(command.barcode());
            if (!Objects.equals(product.getBarcode(), before)) {
                changedFields.add("barcode");
            }
        }
        if (command.unitPrice() != null) {
            BigDecimal before = product.getUnitPrice();
            product.updateUnitPrice(command.unitPrice());
            if (product.getUnitPrice().compareTo(before) != 0) {
                changedFields.add("unitPrice");
            }
        }
        applyCategoryChange(organizationId, product, command.categoryId(), changedFields);
        if (command.status() != null) {
            ProductStatus before = product.getStatus();
            product.updateStatus(command.status());
            if (product.getStatus() != before) {
                changedFields.add("status");
            }
        }

        Product saved = productRepository.saveAndFlush(product);
        if (!changedFields.isEmpty()) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("changedFields", changedFields);
            auditPort.record(AuditRecord.of(
                    organizationId, "product.updated", "product", saved.getId(), AuditMetadata.of(metadata)));
        }
        return saved;
    }

    private void applyCategoryChange(
            UUID organizationId, Product product, UuidPatchField categoryField, List<String> changedFields) {
        if (categoryField == null) {
            return;
        }
        if (categoryField.nullValue()) {
            if (product.getCategory() != null) {
                product.removeCategory();
                changedFields.add("categoryId");
            }
            return;
        }
        Category target = resolveActiveCategory(organizationId, categoryField.value());
        UUID currentId = product.getCategory() == null ? null : product.getCategory().getId();
        if (!target.getId().equals(currentId)) {
            product.assignCategory(target);
            changedFields.add("categoryId");
        }
    }

    private Category resolveActiveCategory(UUID organizationId, UUID categoryId) {
        Category category = categoryRepository
                .findByIdAndOrganizationId(categoryId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.CATEGORY_NOT_FOUND, "Category not found"));
        if (!category.isActive()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.CATEGORY_INACTIVE, "The category is inactive");
        }
        return category;
    }

    private static Specification<Product> buildSpecification(
            UUID organizationId, ProductStatus status, UUID categoryId, String q) {
        return (root, query, cb) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(cb.equal(root.get("organization").get("id"), organizationId));
            if (status != null) {
                predicates.add(cb.equal(root.get("status"), status));
            }
            if (categoryId != null) {
                predicates.add(cb.equal(root.get("category").get("id"), categoryId));
            }
            if (q != null && !q.isBlank()) {
                String trimmed = q.trim();
                predicates.add(cb.or(
                        cb.like(cb.lower(root.get("name")), "%" + trimmed.toLowerCase(Locale.ROOT) + "%"),
                        cb.like(cb.lower(root.get("sku")), "%" + trimmed.toLowerCase(Locale.ROOT) + "%"),
                        cb.like(root.get("barcode"), "%" + trimmed + "%")));
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

    private static String normalizeOptional(String value) {
        if (value == null || value.trim().isEmpty()) {
            return null;
        }
        return value.trim();
    }
}
