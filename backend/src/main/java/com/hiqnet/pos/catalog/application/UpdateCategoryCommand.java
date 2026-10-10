package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.CategoryStatus;

/**
 * Partial update payload. Null fields are left unchanged; {@code code} is
 * immutable and is not represented here.
 */
public record UpdateCategoryCommand(long expectedVersion, String name, CategoryStatus status) {
}
