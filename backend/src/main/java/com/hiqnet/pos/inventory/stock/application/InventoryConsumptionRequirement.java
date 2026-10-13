package com.hiqnet.pos.inventory.stock.application;

import java.math.BigDecimal;
import java.util.UUID;

/**
 * One aggregated Inventory consumption demand for a single sale, derived from
 * the authoritative per-line snapshots at checkout time. The code snapshot is
 * carried only to label safe shortage details.
 */
public record InventoryConsumptionRequirement(
        UUID inventoryItemId,
        String inventoryItemCodeSnapshot,
        String baseUnitSnapshot,
        BigDecimal requiredQuantity) {}
