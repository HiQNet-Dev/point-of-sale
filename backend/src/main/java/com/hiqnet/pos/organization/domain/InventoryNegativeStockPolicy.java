package com.hiqnet.pos.organization.domain;

/**
 * Organization-scoped commercial policy for negative tracked Inventory.
 *
 * <p>{@code STRICT}: a future Sales checkout MUST fail when tracked Inventory
 * is insufficient. {@code ALLOW_NEGATIVE}: a future Sales checkout MAY complete
 * and the branch balance may become negative.</p>
 *
 * <p>Recorded in C3a as configuration only: it is not consumed yet by
 * {@code InventoryStockService} or Sales checkout, and it never changes
 * Opening Stock or manual Adjustments.</p>
 */
public enum InventoryNegativeStockPolicy {
    STRICT,
    ALLOW_NEGATIVE
}
