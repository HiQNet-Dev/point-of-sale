package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.catalog.api.PosProductConfigurationResponse;
import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryRepository;
import com.hiqnet.pos.catalog.domain.CategoryStatus;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductModifierGroup;
import com.hiqnet.pos.catalog.domain.ProductModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.catalog.domain.ProductStatus;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.criteria.Predicate;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.stream.Collectors;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/** Organization-owned catalog reads made available in an ACTIVE Branch authorization context. */
@Service
public class OperationalCatalogService {

    private final BranchRepository branchRepository;
    private final CategoryRepository categoryRepository;
    private final ProductRepository productRepository;
    private final ProductModifierGroupRepository assignmentRepository;
    private final ModifierGroupRepository modifierGroupRepository;
    private final ModifierOptionRepository modifierOptionRepository;

    public OperationalCatalogService(
            BranchRepository branchRepository,
            CategoryRepository categoryRepository,
            ProductRepository productRepository,
            ProductModifierGroupRepository assignmentRepository,
            ModifierGroupRepository modifierGroupRepository,
            ModifierOptionRepository modifierOptionRepository) {
        this.branchRepository = branchRepository;
        this.categoryRepository = categoryRepository;
        this.productRepository = productRepository;
        this.assignmentRepository = assignmentRepository;
        this.modifierGroupRepository = modifierGroupRepository;
        this.modifierOptionRepository = modifierOptionRepository;
    }

    @Transactional(readOnly = true)
    public Page<Category> categories(UUID organizationId, UUID branchId, CategoryStatus status, Pageable pageable) {
        requireActiveBranch(organizationId, branchId);
        return status == null
                ? categoryRepository.findByOrganizationId(organizationId, pageable)
                : categoryRepository.findByOrganizationIdAndStatus(organizationId, status, pageable);
    }

    @Transactional(readOnly = true)
    public Page<Product> products(
            UUID organizationId, UUID branchId, UUID categoryId, String q, Pageable pageable) {
        requireActiveBranch(organizationId, branchId);
        if (categoryId != null) {
            categoryRepository.findByIdAndOrganizationId(categoryId, organizationId)
                    .orElseThrow(() -> new BusinessException(
                            HttpStatus.NOT_FOUND, ErrorCodes.CATEGORY_NOT_FOUND, "Category not found"));
        }
        return productRepository.findAll(productSpecification(organizationId, categoryId, q), pageable);
    }

    @Transactional(readOnly = true)
    public PosProductConfigurationResponse configuration(UUID organizationId, UUID branchId, UUID productId) {
        requireActiveBranch(organizationId, branchId);
        Product product = productRepository.findByIdAndOrganizationId(productId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.PRODUCT_NOT_FOUND, "Product not found"));
        if (!product.isActive()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.PRODUCT_INACTIVE, "The product is inactive");
        }

        List<ProductModifierGroup> assignments = assignmentRepository
                .findByProductIdAndOrganizationIdOrderByDisplayOrderAscModifierGroupIdAsc(productId, organizationId);
        if (assignments.isEmpty()) {
            return PosProductConfigurationResponse.from(product, List.of());
        }

        List<UUID> groupIds = assignments.stream().map(ProductModifierGroup::getModifierGroupId).toList();
        Map<UUID, ModifierGroup> groups = modifierGroupRepository
                .findByOrganizationIdAndIdIn(organizationId, groupIds).stream()
                .collect(Collectors.toMap(ModifierGroup::getId, group -> group));
        Map<UUID, List<ModifierOption>> optionsByGroup = modifierOptionRepository
                .findAllForGroups(organizationId, groupIds).stream()
                .collect(Collectors.groupingBy(option -> option.getModifierGroup().getId()));

        List<PosProductConfigurationResponse.GroupConfiguration> groupResponses = new ArrayList<>();
        for (ProductModifierGroup assignment : assignments) {
            ModifierGroup group = groups.get(assignment.getModifierGroupId());
            if (group == null) {
                continue;
            }
            groupResponses.add(PosProductConfigurationResponse.GroupConfiguration.from(
                    group, assignment.getDisplayOrder(), optionsByGroup.getOrDefault(group.getId(), List.of())));
        }
        return PosProductConfigurationResponse.from(product, groupResponses);
    }

    private void requireActiveBranch(UUID organizationId, UUID branchId) {
        Branch branch = branchRepository.findByIdAndOrganizationId(branchId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found"));
        if (branch.getStatus() != BranchStatus.ACTIVE) {
            throw new BusinessException(HttpStatus.FORBIDDEN, ErrorCodes.FORBIDDEN, "Branch is inactive");
        }
    }

    private static Specification<Product> productSpecification(UUID organizationId, UUID categoryId, String q) {
        return (root, query, cb) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(cb.equal(root.get("organization").get("id"), organizationId));
            predicates.add(cb.equal(root.get("status"), ProductStatus.ACTIVE));
            if (categoryId != null) {
                predicates.add(cb.equal(root.get("category").get("id"), categoryId));
            }
            if (q != null && !q.isBlank()) {
                String trimmed = q.trim();
                String pattern = "%" + trimmed.toLowerCase(Locale.ROOT) + "%";
                predicates.add(cb.or(
                        cb.like(cb.lower(root.get("name")), pattern),
                        cb.like(cb.lower(root.get("sku")), pattern),
                        cb.like(cb.lower(root.get("barcode")), "%" + trimmed + "%")));
            }
            return cb.and(predicates.toArray(new Predicate[0]));
        };
    }
}
