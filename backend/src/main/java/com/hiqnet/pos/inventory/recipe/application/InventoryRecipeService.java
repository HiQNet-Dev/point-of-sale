package com.hiqnet.pos.inventory.recipe.application;

import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.recipe.api.InventoryRecipeComponentResponse;
import com.hiqnet.pos.inventory.recipe.api.ProductInventoryRecipeResponse;
import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeComponentRepository;
import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus;
import com.hiqnet.pos.inventory.recipe.domain.ProductInventoryRecipe;
import com.hiqnet.pos.inventory.recipe.domain.ProductInventoryRecipeRepository;
import com.hiqnet.pos.inventory.recipe.domain.ProductInventoryRecipeStore;
import com.hiqnet.pos.inventory.recipe.domain.RecipeComponentData;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Service
public class InventoryRecipeService {
    private final OrganizationRepository organizationRepository;
    private final ProductRepository productRepository;
    private final InventoryItemRepository inventoryItemRepository;
    private final ProductInventoryRecipeRepository recipeRepository;
    private final ProductInventoryRecipeStore recipeStore;
    private final InventoryRecipeComponentRepository componentRepository;
    private final AuditPort auditPort;

    public InventoryRecipeService(OrganizationRepository organizationRepository, ProductRepository productRepository,
            InventoryItemRepository inventoryItemRepository, ProductInventoryRecipeRepository recipeRepository,
            ProductInventoryRecipeStore recipeStore, InventoryRecipeComponentRepository componentRepository,
            AuditPort auditPort) {
        this.organizationRepository = organizationRepository;
        this.productRepository = productRepository;
        this.inventoryItemRepository = inventoryItemRepository;
        this.recipeRepository = recipeRepository;
        this.recipeStore = recipeStore;
        this.componentRepository = componentRepository;
        this.auditPort = auditPort;
    }

    @Transactional(readOnly = true)
    public ProductInventoryRecipeResponse get(UUID organizationId, UUID productId) {
        requireProduct(organizationId, productId);
        ProductInventoryRecipe recipe = recipeRepository.findByOrganizationIdAndProductId(organizationId, productId)
                .orElseThrow(() -> notFound(ErrorCodes.INVENTORY_RECIPE_NOT_FOUND, "Inventory recipe not found"));
        return response(recipe);
    }

    @Transactional
    public ProductInventoryRecipeResponse replace(UUID organizationId, UUID productId, long expectedVersion,
            InventoryRecipeStatus status, List<InventoryRecipeComponentChange> requested) {
        requireProduct(organizationId, productId);
        if (status == null || requested == null) {
            throw validation("status and components are required");
        }
        if (status == InventoryRecipeStatus.ACTIVE && requested.isEmpty()) {
            throw validation("An ACTIVE inventory recipe must contain at least one component");
        }
        if (requested.stream().anyMatch(change -> change == null || change.inventoryItemId() == null
                || change.quantityPerProduct() == null)) {
            throw validation("Each component requires inventoryItemId and quantity");
        }
        if (new HashSet<>(requested.stream().map(InventoryRecipeComponentChange::inventoryItemId).toList()).size()
                != requested.size()) {
            throw validation("An inventory item may appear only once in a recipe");
        }

        Map<UUID, InventoryItem> items = resolveItems(organizationId,
                requested.stream().map(InventoryRecipeComponentChange::inventoryItemId).toList());
        List<RecipeComponentData> desired = requested.stream()
                .map(change -> {
                    InventoryItem item = items.get(change.inventoryItemId());
                    BigDecimal quantity = change.quantityPerProduct();
                    InventoryQuantity.validateRange(quantity, "quantity");
                    if (quantity.signum() <= 0) {
                        throw invalidQuantity("Recipe component quantity must be greater than zero");
                    }
                    validateUnit(item, quantity);
                    return toComponent(item, quantity);
                })
                .sorted(java.util.Comparator.comparing(RecipeComponentData::inventoryItemId))
                .toList();

        int inserted = recipeStore.ensure(UUID.randomUUID(), organizationId, productId);
        ProductInventoryRecipe recipe = recipeRepository.findByOrganizationIdAndProductIdForUpdate(organizationId, productId)
                .orElseThrow(() -> new IllegalStateException("Inventory recipe row was not initialized"));
        if (recipe.getVersion() != expectedVersion) {
            throw concurrentModification();
        }
        List<RecipeComponentData> existing = componentRepository.findByOrganizationIdAndRecipeId(organizationId, recipe.getId());
        validateItemStatus(desired, existing, status);
        if (recipe.getStatus() == status && sameComponents(existing, desired)) {
            return ProductInventoryRecipeResponse.from(recipe, existing.stream()
                    .map(InventoryRecipeComponentResponse::from).toList());
        }

        boolean statusChanged = recipe.getStatus() != status;
        componentRepository.replace(organizationId, recipe.getId(), desired);
        recipe.replace(status);
        ProductInventoryRecipe saved = recipeRepository.saveAndFlush(recipe);
        String action = inserted == 1 ? "inventory.recipe.created"
                : statusChanged ? "inventory.recipe.status_changed" : "inventory.recipe.updated";
        auditPort.record(AuditRecord.of(organizationId, action, "product_inventory_recipe", saved.getId(),
                AuditMetadata.of(Map.of("productId", productId.toString(), "status", status.name(),
                        "componentCount", desired.size()))));
        return ProductInventoryRecipeResponse.from(saved, desired.stream().map(InventoryRecipeComponentResponse::from).toList());
    }

    private Map<UUID, InventoryItem> resolveItems(UUID organizationId, List<UUID> ids) {
        if (ids.isEmpty()) return Map.of();
        List<InventoryItem> found = inventoryItemRepository.findByOrganizationIdAndIdIn(organizationId, ids);
        if (found.size() != ids.size()) {
            throw notFound(ErrorCodes.INVENTORY_ITEM_NOT_FOUND,
                    "One or more inventory items were not found in the organization");
        }
        Map<UUID, InventoryItem> result = new HashMap<>();
        found.forEach(item -> result.put(item.getId(), item));
        return result;
    }

    private void validateItemStatus(List<RecipeComponentData> desired, List<RecipeComponentData> existing,
            InventoryRecipeStatus requestedStatus) {
        Map<UUID, RecipeComponentData> previous = new HashMap<>();
        existing.forEach(component -> previous.put(component.inventoryItemId(), component));
        for (RecipeComponentData component : desired) {
            if (component.status() == InventoryItemStatus.ACTIVE) continue;
            RecipeComponentData prior = previous.get(component.inventoryItemId());
            boolean retainedUnchangedWhileInactive = requestedStatus == InventoryRecipeStatus.INACTIVE
                    && prior != null && prior.quantityPerProduct().compareTo(component.quantityPerProduct()) == 0;
            if (!retainedUnchangedWhileInactive) {
                throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.INVENTORY_ITEM_INACTIVE,
                        "New recipe components must reference active inventory items; remove inactive items before activation");
            }
        }
    }

    private static boolean sameComponents(List<RecipeComponentData> left, List<RecipeComponentData> right) {
        if (left.size() != right.size()) return false;
        for (int index = 0; index < left.size(); index++) {
            if (!left.get(index).inventoryItemId().equals(right.get(index).inventoryItemId())
                    || left.get(index).quantityPerProduct().compareTo(right.get(index).quantityPerProduct()) != 0) return false;
        }
        return true;
    }

    private ProductInventoryRecipeResponse response(ProductInventoryRecipe recipe) {
        List<InventoryRecipeComponentResponse> components = componentRepository
                .findByOrganizationIdAndRecipeId(recipe.getOrganizationId(), recipe.getId()).stream()
                .map(InventoryRecipeComponentResponse::from).toList();
        return ProductInventoryRecipeResponse.from(recipe, components);
    }

    private static RecipeComponentData toComponent(InventoryItem item, BigDecimal quantity) {
        return new RecipeComponentData(item.getId(), item.getCode(), item.getName(), item.getBaseUnit(),
                item.isFractionalUnitAllowed(), item.getStatus(), quantity);
    }

    private Product requireProduct(UUID organizationId, UUID productId) {
        if (!organizationRepository.existsById(organizationId)) {
            throw notFound(ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found");
        }
        return productRepository.findByIdAndOrganizationId(productId, organizationId)
                .orElseThrow(() -> notFound(ErrorCodes.PRODUCT_NOT_FOUND, "Product not found"));
    }

    private static void validateUnit(InventoryItem item, BigDecimal quantity) {
        if (item.getBaseUnit() == InventoryBaseUnit.UNIT && !item.isFractionalUnitAllowed()
                && quantity.stripTrailingZeros().scale() > 0) {
            throw invalidQuantity("Fractional quantities are not allowed for this UNIT item");
        }
    }

    private static BusinessException concurrentModification() {
        return new BusinessException(HttpStatus.CONFLICT, ErrorCodes.CONCURRENT_MODIFICATION,
                "The recipe was modified concurrently; retry with fresh data");
    }
    private static BusinessException invalidQuantity(String message) {
        return new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_INVENTORY_QUANTITY, message);
    }
    private static BusinessException validation(String message) {
        return new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, message);
    }
    private static BusinessException notFound(String code, String message) {
        return new BusinessException(HttpStatus.NOT_FOUND, code, message);
    }
}
