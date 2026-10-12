package com.hiqnet.pos.inventory.consumption;

import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.modifier.domain.ModifierInventoryEffectData;
import com.hiqnet.pos.inventory.modifier.domain.ModifierOptionInventoryEffectRepository;
import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeComponentRepository;
import com.hiqnet.pos.inventory.recipe.domain.ProductInventoryRecipe;
import com.hiqnet.pos.inventory.recipe.domain.ProductInventoryRecipeRepository;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/** Resolves organization-owned recipe plus caller-validated selected options; it never reads or mutates branch stock. */
@Service
public class InventoryConsumptionResolver {
    private final ProductRepository productRepository;
    private final ModifierOptionRepository modifierOptionRepository;
    private final ProductInventoryRecipeRepository recipeRepository;
    private final InventoryRecipeComponentRepository componentRepository;
    private final ModifierOptionInventoryEffectRepository effectRepository;

    public InventoryConsumptionResolver(ProductRepository productRepository,
            ModifierOptionRepository modifierOptionRepository, ProductInventoryRecipeRepository recipeRepository,
            InventoryRecipeComponentRepository componentRepository,
            ModifierOptionInventoryEffectRepository effectRepository) {
        this.productRepository = productRepository;
        this.modifierOptionRepository = modifierOptionRepository;
        this.recipeRepository = recipeRepository;
        this.componentRepository = componentRepository;
        this.effectRepository = effectRepository;
    }

    @Transactional(readOnly = true)
    public ResolvedInventoryConsumption resolve(UUID organizationId, UUID productId,
            Collection<UUID> validatedSelectedModifierOptionIds) {
        Product product = productRepository.findByIdAndOrganizationId(productId, organizationId)
                .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.PRODUCT_NOT_FOUND,
                        "Product not found"));
        ProductInventoryRecipe recipe = recipeRepository.findByOrganizationIdAndProductId(organizationId, productId)
                .orElse(null);
        if (recipe == null || recipe.getStatus() != com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus.ACTIVE) {
            return ResolvedInventoryConsumption.untracked();
        }

        List<UUID> optionIds = validatedSelectedModifierOptionIds == null
                ? List.of() : validatedSelectedModifierOptionIds.stream().distinct().toList();
        if (optionIds.stream().anyMatch(java.util.Objects::isNull)) {
            throw unavailable("Selected modifier option is missing");
        }
        if (!optionIds.isEmpty()) {
            List<ModifierOption> options = modifierOptionRepository.findByOrganizationIdAndIdIn(organizationId, optionIds);
            if (options.size() != optionIds.size()) {
                throw unavailable("Selected modifier option is not available in this organization");
            }
        }

        Map<UUID, MutableComponent> totals = new HashMap<>();
        componentRepository.findByOrganizationIdAndRecipeId(organizationId, recipe.getId()).forEach(component -> {
            requireActive(component.status(), component.inventoryItemId());
            add(totals, component.inventoryItemId(), component.code(), component.name(), component.baseUnit(),
                    component.fractionalUnitAllowed(), component.quantityPerProduct());
        });
        List<ModifierInventoryEffectData> effects = effectRepository
                .findByOrganizationIdAndModifierOptionIds(organizationId, optionIds);
        for (ModifierInventoryEffectData effect : effects) {
            requireActive(effect.status(), effect.inventoryItemId());
            add(totals, effect.inventoryItemId(), effect.code(), effect.name(), effect.baseUnit(),
                    effect.fractionalUnitAllowed(), effect.quantityDelta());
        }

        List<ResolvedInventoryComponent> resolved = new ArrayList<>();
        for (MutableComponent component : totals.values()) {
            if (component.quantity.signum() < 0) {
                throw unavailable("Resolved Inventory consumption must not be negative");
            }
            try {
                InventoryQuantity.validateRange(component.quantity, "resolved quantity");
            } catch (BusinessException exception) {
                throw unavailable("Resolved Inventory consumption exceeds its supported range");
            }
            if (component.quantity.signum() == 0) continue;
            if (component.baseUnit == InventoryBaseUnit.UNIT && !component.fractionalUnitAllowed
                    && component.quantity.stripTrailingZeros().scale() > 0) {
                throw unavailable("Resolved Inventory consumption is fractional for a discrete UNIT item");
            }
            resolved.add(new ResolvedInventoryComponent(component.id, component.code, component.name,
                    component.baseUnit, component.fractionalUnitAllowed, component.quantity));
        }
        resolved.sort(Comparator.comparing(ResolvedInventoryComponent::inventoryItemId));
        return new ResolvedInventoryConsumption(true, resolved);
    }

    private static void add(Map<UUID, MutableComponent> totals, UUID id, String code, String name,
            InventoryBaseUnit unit, boolean fractionalAllowed, BigDecimal quantity) {
        MutableComponent current = totals.get(id);
        if (current == null) {
            totals.put(id, new MutableComponent(id, code, name, unit, fractionalAllowed, quantity));
        } else {
            current.quantity = current.quantity.add(quantity);
        }
    }

    private static void requireActive(InventoryItemStatus status, UUID itemId) {
        if (status != InventoryItemStatus.ACTIVE) {
            throw unavailable("Inventory item " + itemId + " is inactive and must be corrected before consumption resolution");
        }
    }

    private static BusinessException unavailable(String message) {
        return new BusinessException(HttpStatus.CONFLICT, ErrorCodes.INVENTORY_CONFIGURATION_UNAVAILABLE, message);
    }

    private static final class MutableComponent {
        private final UUID id;
        private final String code;
        private final String name;
        private final InventoryBaseUnit baseUnit;
        private final boolean fractionalUnitAllowed;
        private BigDecimal quantity;

        private MutableComponent(UUID id, String code, String name, InventoryBaseUnit baseUnit,
                boolean fractionalUnitAllowed, BigDecimal quantity) {
            this.id = id;
            this.code = code;
            this.name = name;
            this.baseUnit = baseUnit;
            this.fractionalUnitAllowed = fractionalUnitAllowed;
            this.quantity = quantity;
        }
    }
}
