package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.CategoryStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

/**
 * Partial update: only fields present are applied; {@code code} is immutable
 * and is not accepted here. {@code expectedVersion} is required (the version
 * the client read).
 */
public record UpdateCategoryRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "0")
        @NotNull
                @Min(0)
                Long expectedVersion,
        @Size(max = 120) String name,
        CategoryStatus status) {
}
