package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.ProductStatus;
import com.hiqnet.pos.shared.api.UuidPatchField;
import java.math.BigDecimal;

/**
 * Application command for a product partial update. {@code categoryId} carries
 * the tri-state semantics (null = no change, present-null = detach, value =
 * assign).
 */
public record UpdateProductCommand(
        long expectedVersion,
        String barcode,
        String name,
        String description,
        UuidPatchField categoryId,
        BigDecimal unitPrice,
        ProductStatus status) {
}
