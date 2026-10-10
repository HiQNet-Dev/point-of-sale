package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryStatus;
import java.time.Instant;
import java.util.UUID;

public record CategoryResponse(
        UUID id,
        UUID organizationId,
        String code,
        String name,
        CategoryStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static CategoryResponse from(Category category) {
        return new CategoryResponse(
                category.getId(),
                category.getOrganization().getId(),
                category.getCode(),
                category.getName(),
                category.getStatus(),
                category.getVersion(),
                category.getCreatedAt(),
                category.getUpdatedAt());
    }
}
