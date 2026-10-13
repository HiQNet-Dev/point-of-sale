package com.hiqnet.pos.inventory.stock.domain;

import java.math.BigDecimal;
import java.util.UUID;

/**
 * One append-only stock fact to persist in a single batch. {@code saleId} is
 * required for SALE_CONSUMPTION and null for OPENING/ADJUSTMENT, mirroring the
 * database coupling constraint.
 */
public record InventoryMovementInsert(
        UUID id,
        UUID organizationId,
        UUID branchId,
        UUID inventoryItemId,
        InventoryMovementType type,
        BigDecimal delta,
        BigDecimal resultingQuantity,
        String reason,
        UUID actorUserId,
        UUID saleId) {}
