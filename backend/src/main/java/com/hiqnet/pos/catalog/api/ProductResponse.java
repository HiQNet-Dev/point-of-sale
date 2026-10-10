package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductStatus;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

public record ProductResponse(
        UUID id,
        UUID organizationId,
        UUID categoryId,
        String sku,
        String barcode,
        String name,
        String description,
        BigDecimal unitPrice,
        ProductStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static ProductResponse from(Product product) {
        return new ProductResponse(
                product.getId(),
                product.getOrganization().getId(),
                product.getCategory() == null ? null : product.getCategory().getId(),
                product.getSku(),
                product.getBarcode(),
                product.getName(),
                product.getDescription(),
                product.getUnitPrice(),
                product.getStatus(),
                product.getVersion(),
                product.getCreatedAt(),
                product.getUpdatedAt());
    }
}
