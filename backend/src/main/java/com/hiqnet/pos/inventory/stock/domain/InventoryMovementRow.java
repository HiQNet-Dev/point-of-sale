package com.hiqnet.pos.inventory.stock.domain;

import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;

public record InventoryMovementRow(
        UUID id,
        InventoryMovementType type,
        BigDecimal quantityDelta,
        BigDecimal resultingQuantity,
        String reason,
        UUID actorUserId,
        Instant occurredAt) {}
