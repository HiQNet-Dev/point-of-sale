package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductStatus;
import java.math.BigDecimal;
import java.util.UUID;

/** Display-only product data for POS discovery; Sale remains the pricing authority. */
public record PosProductResponse(
        UUID id,
        String sku,
        String barcode,
        String name,
        String description,
        BigDecimal unitPrice,
        UUID categoryId,
        ProductStatus status) {

    public static PosProductResponse from(Product product) {
        return new PosProductResponse(
                product.getId(),
                product.getSku(),
                product.getBarcode(),
                product.getName(),
                product.getDescription(),
                product.getUnitPrice(),
                product.getCategory() == null ? null : product.getCategory().getId(),
                product.getStatus());
    }
}
