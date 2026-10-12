-- Organization-scoped negative stock policy.
-- Configuration only: no Checkout, BranchInventory or InventoryMovement change,
-- and no SALE_CONSUMPTION movement. Consumed by the future Sales checkout stock
-- consumption (C3b). Manual adjustments and opening stock are unaffected.
-- The DEFAULT 'STRICT' is kept as a defense for direct/administrative inserts,
-- mirroring organizations.status DEFAULT 'ACTIVE'; existing rows become STRICT.

ALTER TABLE organizations
    ADD COLUMN inventory_negative_stock_policy VARCHAR(16) NOT NULL DEFAULT 'STRICT'
        CHECK (inventory_negative_stock_policy IN ('STRICT', 'ALLOW_NEGATIVE'));

COMMENT ON COLUMN organizations.inventory_negative_stock_policy IS
    'Organization-scoped negative stock policy for future Sales checkout consumption. STRICT blocks consumption when tracked Inventory is insufficient; ALLOW_NEGATIVE permits the branch balance to go negative. Manual adjustments and opening stock are unaffected. Default STRICT.';
