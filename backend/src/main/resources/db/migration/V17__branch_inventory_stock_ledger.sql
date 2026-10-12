-- Branch-owned materialized balances and immutable stock history. Existing role grants are preserved.
CREATE TABLE branch_inventory (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    branch_id UUID NOT NULL,
    inventory_item_id UUID NOT NULL,
    quantity_on_hand NUMERIC(19,6) NOT NULL DEFAULT 0,
    version BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT branch_inventory_branch_item_unique UNIQUE (branch_id, inventory_item_id),
    CONSTRAINT branch_inventory_id_organization_unique UNIQUE (id, organization_id),
    CONSTRAINT branch_inventory_branch_fk FOREIGN KEY (branch_id, organization_id) REFERENCES branches (id, organization_id),
    CONSTRAINT branch_inventory_item_fk FOREIGN KEY (inventory_item_id, organization_id) REFERENCES inventory_items (id, organization_id)
);

CREATE TABLE inventory_movements (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    branch_id UUID NOT NULL,
    inventory_item_id UUID NOT NULL,
    type VARCHAR(16) NOT NULL CHECK (type IN ('OPENING', 'ADJUSTMENT')),
    quantity_delta NUMERIC(19,6) NOT NULL,
    resulting_quantity NUMERIC(19,6) NOT NULL,
    reason VARCHAR(255),
    actor_user_id UUID NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT inventory_movements_branch_fk FOREIGN KEY (branch_id, organization_id) REFERENCES branches (id, organization_id),
    CONSTRAINT inventory_movements_item_fk FOREIGN KEY (inventory_item_id, organization_id) REFERENCES inventory_items (id, organization_id),
    CONSTRAINT inventory_movements_actor_fk FOREIGN KEY (actor_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT inventory_movements_type_values CHECK (
        (type = 'OPENING' AND quantity_delta >= 0)
        OR (type = 'ADJUSTMENT' AND quantity_delta <> 0 AND reason IS NOT NULL AND length(trim(reason)) > 0)
    )
);

CREATE UNIQUE INDEX inventory_movements_one_opening_per_branch_item
    ON inventory_movements (organization_id, branch_id, inventory_item_id) WHERE type = 'OPENING';
CREATE INDEX idx_inventory_movements_branch_item_occurred
    ON inventory_movements (organization_id, branch_id, inventory_item_id, occurred_at DESC, id DESC);

CREATE OR REPLACE FUNCTION inventory_movements_reject_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'inventory_movements is append-only: % is not allowed', TG_OP USING ERRCODE = 'check_violation';
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER inventory_movements_append_only BEFORE UPDATE OR DELETE ON inventory_movements
    FOR EACH ROW EXECUTE FUNCTION inventory_movements_reject_mutation();

INSERT INTO permissions (code, description) VALUES
    ('inventory.stock.read', 'Read branch inventory balances and movement history'),
    ('inventory.stock.adjust', 'Open stock and make manual branch inventory adjustments');

COMMENT ON TABLE branch_inventory IS 'Lazy materialized current quantity for one branch and organization-owned inventory item. Missing row reads as zero/version zero.';
COMMENT ON TABLE inventory_movements IS 'Immutable append-only stock facts. Only OPENING and ADJUSTMENT are supported in V17.';
