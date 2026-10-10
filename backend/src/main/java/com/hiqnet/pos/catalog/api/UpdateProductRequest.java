package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.ProductStatus;
import com.hiqnet.pos.shared.api.UuidPatchField;
import jakarta.validation.constraints.DecimalMin;
import jakarta.validation.constraints.Digits;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;
import java.math.BigDecimal;

/**
 * Partial update of a product. {@code sku} is immutable and intentionally
 * absent. {@code categoryId} is a tri-state field: omitted = no change,
 * {@code null} = detach the category, value = assign that category (must exist
 * in the same organization and be ACTIVE).
 */
public record UpdateProductRequest(
        @NotNull @Min(0) Long expectedVersion,
        @Size(max = 64) String barcode,
        @Size(max = 120) String name,
        @Size(max = 1000) String description,
        UuidPatchField categoryId,
        @DecimalMin(value = "0") @Digits(integer = 15, fraction = 4) BigDecimal unitPrice,
        ProductStatus status) {
}
