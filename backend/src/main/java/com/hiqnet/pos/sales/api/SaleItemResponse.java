package com.hiqnet.pos.sales.api;

import com.hiqnet.pos.sales.domain.SaleItem;
import com.hiqnet.pos.sales.domain.SaleItemModifier;
import com.hiqnet.pos.sales.domain.SaleItemStatus;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.List;
import java.util.UUID;

public record SaleItemResponse(UUID id, UUID productId, String productSkuSnapshot, String productNameSnapshot,
        BigDecimal baseUnitPrice, BigDecimal modifierUnitDelta, BigDecimal effectiveUnitPrice, int quantity,
        BigDecimal lineTotal, SaleItemStatus status, int position, Instant createdAt, Instant updatedAt,
        Instant removedAt, UUID removedByUserId, List<SaleItemModifierResponse> modifiers) {
    static SaleItemResponse from(SaleItem item, List<SaleItemModifier> modifiers) {
        return new SaleItemResponse(item.getId(), item.getProductId(), item.getProductSkuSnapshot(), item.getProductNameSnapshot(),
                item.getBaseUnitPrice(), item.getModifierUnitDelta(), item.getEffectiveUnitPrice(), item.getQuantity(),
                item.getLineTotal(), item.getStatus(), item.getPosition(), item.getCreatedAt(), item.getUpdatedAt(),
                item.getRemovedAt(), item.getRemovedByUserId(), modifiers.stream().map(SaleItemModifierResponse::from).toList());
    }
}
