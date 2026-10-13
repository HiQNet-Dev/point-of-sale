-- Atomic checkout Inventory consumption (C3b).
-- Extends the append-only stock ledger so a completed sale can record one
-- SALE_CONSUMPTION movement per consumed Inventory item, tied to the sale.
-- V1-V20 are immutable; existing OPENING/ADJUSTMENT rows keep sale_id NULL.

-- 1. Sales identity superkey required by the branch-scoped movement FK below.
ALTER TABLE sales
    ADD CONSTRAINT sales_id_organization_branch_unique UNIQUE (id, organization_id, branch_id);

-- 2. Link movements to the originating sale (NULL for OPENING/ADJUSTMENT).
ALTER TABLE inventory_movements
    ADD COLUMN sale_id UUID;

ALTER TABLE inventory_movements
    ADD CONSTRAINT inventory_movements_sale_fk
        FOREIGN KEY (sale_id, organization_id, branch_id)
        REFERENCES sales (id, organization_id, branch_id);

-- 3. Extend the allowed type domain (V17 inline CHECK constrains type to OPENING/ADJUSTMENT).
ALTER TABLE inventory_movements
    DROP CONSTRAINT inventory_movements_type_check;
ALTER TABLE inventory_movements
    ADD CONSTRAINT inventory_movements_type_check
        CHECK (type IN ('OPENING', 'ADJUSTMENT', 'SALE_CONSUMPTION'));

-- 4. Per-type invariants. OPENING/ADJUSTMENT keep their V17 semantics and must
--    never carry a sale; SALE_CONSUMPTION is always negative and always linked.
ALTER TABLE inventory_movements
    DROP CONSTRAINT inventory_movements_type_values;
ALTER TABLE inventory_movements
    ADD CONSTRAINT inventory_movements_type_values CHECK (
        (type = 'OPENING' AND quantity_delta >= 0 AND sale_id IS NULL)
        OR (type = 'ADJUSTMENT' AND quantity_delta <> 0 AND reason IS NOT NULL
            AND length(trim(reason)) > 0 AND sale_id IS NULL)
        OR (type = 'SALE_CONSUMPTION' AND quantity_delta < 0 AND sale_id IS NOT NULL)
    );

-- 5. Exactly one consumption movement per sale and Inventory item.
CREATE UNIQUE INDEX inventory_movements_one_consumption_per_sale_item
    ON inventory_movements (sale_id, inventory_item_id) WHERE type = 'SALE_CONSUMPTION';
CREATE INDEX idx_inventory_movements_sale
    ON inventory_movements (sale_id) WHERE sale_id IS NOT NULL;

COMMENT ON COLUMN inventory_movements.sale_id IS
    'Originating sale for SALE_CONSUMPTION movements; NULL for OPENING and ADJUSTMENT.';
COMMENT ON TABLE inventory_movements IS
    'Immutable append-only stock facts: OPENING and ADJUSTMENT (manual), SALE_CONSUMPTION (checkout).';
