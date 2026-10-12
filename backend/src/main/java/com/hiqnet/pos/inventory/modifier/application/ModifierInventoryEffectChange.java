package com.hiqnet.pos.inventory.modifier.application;

import java.math.BigDecimal;
import java.util.UUID;

public record ModifierInventoryEffectChange(UUID inventoryItemId, BigDecimal quantityDelta) {}
