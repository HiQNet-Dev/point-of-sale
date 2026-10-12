package com.hiqnet.pos.inventory.modifier.application;

import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.modifier.api.ModifierInventoryEffectResponse;
import com.hiqnet.pos.inventory.modifier.api.ModifierInventoryEffectsResponse;
import com.hiqnet.pos.inventory.modifier.domain.ModifierInventoryEffectData;
import com.hiqnet.pos.inventory.modifier.domain.ModifierOptionInventoryConfig;
import com.hiqnet.pos.inventory.modifier.domain.ModifierOptionInventoryConfigRepository;
import com.hiqnet.pos.inventory.modifier.domain.ModifierOptionInventoryConfigStore;
import com.hiqnet.pos.inventory.modifier.domain.ModifierOptionInventoryEffectRepository;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Service
public class ModifierInventoryEffectsService {
    private final OrganizationRepository organizationRepository;
    private final ModifierOptionRepository modifierOptionRepository;
    private final InventoryItemRepository inventoryItemRepository;
    private final ModifierOptionInventoryConfigRepository configRepository;
    private final ModifierOptionInventoryConfigStore configStore;
    private final ModifierOptionInventoryEffectRepository effectRepository;
    private final AuditPort auditPort;

    public ModifierInventoryEffectsService(OrganizationRepository organizationRepository,
            ModifierOptionRepository modifierOptionRepository, InventoryItemRepository inventoryItemRepository,
            ModifierOptionInventoryConfigRepository configRepository, ModifierOptionInventoryConfigStore configStore,
            ModifierOptionInventoryEffectRepository effectRepository, AuditPort auditPort) {
        this.organizationRepository = organizationRepository;
        this.modifierOptionRepository = modifierOptionRepository;
        this.inventoryItemRepository = inventoryItemRepository;
        this.configRepository = configRepository;
        this.configStore = configStore;
        this.effectRepository = effectRepository;
        this.auditPort = auditPort;
    }

    @Transactional(readOnly = true)
    public ModifierInventoryEffectsResponse get(UUID organizationId, UUID optionId) {
        requireOption(organizationId, optionId);
        ModifierOptionInventoryConfig config = configRepository
                .findByOrganizationIdAndModifierOptionId(organizationId, optionId)
                .orElseThrow(() -> notFound(ErrorCodes.INVENTORY_MODIFIER_CONFIG_NOT_FOUND,
                        "Modifier option Inventory effects are not configured"));
        return response(config);
    }

    @Transactional
    public ModifierInventoryEffectsResponse replace(UUID organizationId, UUID optionId, long expectedVersion,
            List<ModifierInventoryEffectChange> requested) {
        requireOption(organizationId, optionId);
        if (requested == null || requested.stream().anyMatch(effect -> effect == null
                || effect.inventoryItemId() == null || effect.quantityDelta() == null)) {
            throw validation("Each effect requires inventoryItemId and quantityDelta");
        }
        if (new HashSet<>(requested.stream().map(ModifierInventoryEffectChange::inventoryItemId).toList()).size()
                != requested.size()) {
            throw validation("An inventory item may appear only once in a modifier option effect");
        }
        Map<UUID, InventoryItem> items = resolveItems(organizationId,
                requested.stream().map(ModifierInventoryEffectChange::inventoryItemId).toList());
        List<ModifierInventoryEffectData> desired = requested.stream().map(change -> {
            InventoryItem item = items.get(change.inventoryItemId());
            BigDecimal delta = change.quantityDelta();
            InventoryQuantity.validateRange(delta, "quantityDelta");
            if (delta.signum() == 0) {
                throw invalidQuantity("Modifier inventory effect quantityDelta must not be zero");
            }
            validateUnit(item, delta);
            return toEffect(optionId, item, delta);
        }).sorted(java.util.Comparator.comparing(effect -> effect.inventoryItemId().toString())).toList();

        int inserted = configStore.ensure(UUID.randomUUID(), organizationId, optionId);
        ModifierOptionInventoryConfig config = configRepository
                .findByOrganizationIdAndModifierOptionIdForUpdate(organizationId, optionId)
                .orElseThrow(() -> new IllegalStateException("Modifier Inventory config row was not initialized"));
        if (config.getVersion() != expectedVersion) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.CONCURRENT_MODIFICATION,
                    "Modifier Inventory effects changed; retry with fresh data");
        }
        List<ModifierInventoryEffectData> existing = effectRepository
                .findByOrganizationIdAndModifierOptionId(organizationId, optionId);
        validateItemStatus(desired, existing);
        if (inserted == 0 && sameEffects(existing, desired)) {
            return ModifierInventoryEffectsResponse.from(config, existing.stream()
                    .map(ModifierInventoryEffectResponse::from).toList());
        }

        effectRepository.replace(organizationId, config.getId(), desired);
        config.replace();
        ModifierOptionInventoryConfig saved = configRepository.saveAndFlush(config);
        auditPort.record(AuditRecord.of(organizationId, "inventory.modifier_effects.updated",
                "modifier_option_inventory_config", saved.getId(), AuditMetadata.of(Map.of(
                        "modifierOptionId", optionId.toString(), "effectCount", desired.size(),
                        "created", inserted == 1))));
        return ModifierInventoryEffectsResponse.from(saved,
                desired.stream().map(ModifierInventoryEffectResponse::from).toList());
    }

    private Map<UUID, InventoryItem> resolveItems(UUID organizationId, List<UUID> ids) {
        if (ids.isEmpty()) return Map.of();
        List<InventoryItem> found = inventoryItemRepository.findByOrganizationIdAndIdIn(organizationId, ids);
        if (found.size() != ids.size()) {
            throw notFound(ErrorCodes.INVENTORY_ITEM_NOT_FOUND,
                    "One or more inventory items were not found in the organization");
        }
        Map<UUID, InventoryItem> items = new HashMap<>();
        found.forEach(item -> items.put(item.getId(), item));
        return items;
    }

    private static void validateItemStatus(List<ModifierInventoryEffectData> desired,
            List<ModifierInventoryEffectData> existing) {
        Map<UUID, ModifierInventoryEffectData> previous = new HashMap<>();
        existing.forEach(effect -> previous.put(effect.inventoryItemId(), effect));
        for (ModifierInventoryEffectData effect : desired) {
            if (effect.status() == InventoryItemStatus.ACTIVE) continue;
            ModifierInventoryEffectData prior = previous.get(effect.inventoryItemId());
            if (prior == null || prior.quantityDelta().compareTo(effect.quantityDelta()) != 0) {
                throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.INVENTORY_ITEM_INACTIVE,
                        "New modifier effects must reference active inventory items");
            }
        }
    }

    private static boolean sameEffects(List<ModifierInventoryEffectData> left, List<ModifierInventoryEffectData> right) {
        if (left.size() != right.size()) return false;
        for (int index = 0; index < left.size(); index++) {
            if (!left.get(index).inventoryItemId().equals(right.get(index).inventoryItemId())
                    || left.get(index).quantityDelta().compareTo(right.get(index).quantityDelta()) != 0) return false;
        }
        return true;
    }

    private ModifierInventoryEffectsResponse response(ModifierOptionInventoryConfig config) {
        return ModifierInventoryEffectsResponse.from(config, effectRepository
                .findByOrganizationIdAndModifierOptionId(config.getOrganizationId(), config.getModifierOptionId())
                .stream().map(ModifierInventoryEffectResponse::from).toList());
    }

    private static ModifierInventoryEffectData toEffect(UUID optionId, InventoryItem item, BigDecimal delta) {
        return new ModifierInventoryEffectData(optionId, item.getId(), item.getCode(), item.getName(),
                item.getBaseUnit(), item.isFractionalUnitAllowed(), item.getStatus(), delta);
    }

    private ModifierOption requireOption(UUID organizationId, UUID optionId) {
        if (!organizationRepository.existsById(organizationId)) {
            throw notFound(ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found");
        }
        return modifierOptionRepository.findByIdAndOrganizationId(optionId, organizationId)
                .orElseThrow(() -> notFound(ErrorCodes.MODIFIER_OPTION_NOT_FOUND, "Modifier option not found"));
    }

    private static void validateUnit(InventoryItem item, BigDecimal delta) {
        if (item.getBaseUnit() == InventoryBaseUnit.UNIT && !item.isFractionalUnitAllowed()
                && delta.stripTrailingZeros().scale() > 0) {
            throw invalidQuantity("Fractional effects are not allowed for this UNIT item");
        }
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
