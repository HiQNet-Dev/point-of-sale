package com.hiqnet.pos.sales.domain;

import java.math.BigDecimal;
import java.util.UUID;

/** Historical, aggregated Inventory consumption belonging to one quoted SaleItem. */
public record SaleItemInventoryComponent(
        UUID inventoryItemId,
        String inventoryItemCodeSnapshot,
        String inventoryItemNameSnapshot,
        String baseUnitSnapshot,
        BigDecimal quantityPerUnit,
        BigDecimal quantityTotal) {}
