package com.hiqnet.pos.inventory.modifier.api;

import com.hiqnet.pos.inventory.modifier.domain.ModifierOptionInventoryConfig;
import java.time.Instant;
import java.util.List;
import java.util.UUID;

public record ModifierInventoryEffectsResponse(UUID id, UUID organizationId, UUID modifierOptionId,
        long version, Instant createdAt, Instant updatedAt, List<ModifierInventoryEffectResponse> effects) {
    public static ModifierInventoryEffectsResponse from(ModifierOptionInventoryConfig config,
            List<ModifierInventoryEffectResponse> effects) {
        return new ModifierInventoryEffectsResponse(config.getId(), config.getOrganizationId(),
                config.getModifierOptionId(), config.getVersion(), config.getCreatedAt(), config.getUpdatedAt(), List.copyOf(effects));
    }
}
