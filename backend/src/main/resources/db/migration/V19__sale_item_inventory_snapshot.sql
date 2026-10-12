CREATE TABLE sale_item_inventory_components (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    sale_item_id UUID NOT NULL,
    inventory_item_id UUID NOT NULL,
    inventory_item_code_snapshot VARCHAR(64) NOT NULL,
    inventory_item_name_snapshot VARCHAR(120) NOT NULL CHECK (length(trim(inventory_item_name_snapshot)) > 0),
    base_unit_snapshot VARCHAR(16) NOT NULL CHECK (base_unit_snapshot IN ('UNIT', 'GRAM', 'MILLILITER')),
    quantity_per_unit NUMERIC(19,6) NOT NULL CHECK (quantity_per_unit > 0),
    quantity_total NUMERIC(19,6) NOT NULL CHECK (quantity_total > 0),
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT sale_item_inventory_components_item_fk
        FOREIGN KEY (sale_item_id, organization_id) REFERENCES sale_items (id, organization_id) ON DELETE CASCADE,
    CONSTRAINT sale_item_inventory_components_inventory_item_fk
        FOREIGN KEY (inventory_item_id, organization_id) REFERENCES inventory_items (id, organization_id),
    CONSTRAINT sale_item_inventory_components_item_unique UNIQUE (sale_item_id, inventory_item_id)
);

COMMENT ON TABLE sale_item_inventory_components IS
    'Sales-owned historical Inventory consumption resolved when an active SaleItem is created or materially requoted. No recipe backfill; checkout stock deduction is deferred.';
COMMENT ON COLUMN sale_item_inventory_components.quantity_per_unit IS
    'Exact resolved Inventory consumption for one unit of the quoted product.';
COMMENT ON COLUMN sale_item_inventory_components.quantity_total IS
    'Exact quantity_per_unit multiplied by the SaleItem integer quantity at quote time.';
