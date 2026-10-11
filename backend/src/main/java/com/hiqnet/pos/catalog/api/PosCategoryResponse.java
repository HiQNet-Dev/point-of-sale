package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryStatus;
import java.util.UUID;

/** Minimal category data for operational catalog discovery. */
public record PosCategoryResponse(UUID id, String code, String name, CategoryStatus status) {

    public static PosCategoryResponse from(Category category) {
        return new PosCategoryResponse(
                category.getId(), category.getCode(), category.getName(), category.getStatus());
    }
}
