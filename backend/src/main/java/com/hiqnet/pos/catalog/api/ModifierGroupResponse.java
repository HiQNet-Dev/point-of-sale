package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;
import java.time.Instant;
import java.util.UUID;

public record ModifierGroupResponse(
        UUID id,
        UUID organizationId,
        String code,
        String name,
        int minSelections,
        int maxSelections,
        boolean required,
        boolean singleChoice,
        boolean multipleChoice,
        ModifierGroupStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static ModifierGroupResponse from(ModifierGroup group) {
        return new ModifierGroupResponse(
                group.getId(),
                group.getOrganization().getId(),
                group.getCode(),
                group.getName(),
                group.getMinSelections(),
                group.getMaxSelections(),
                group.isRequired(),
                group.isSingleChoice(),
                group.isMultipleChoice(),
                group.getStatus(),
                group.getVersion(),
                group.getCreatedAt(),
                group.getUpdatedAt());
    }
}
