-- Sales Foundation: an operational sale belongs to exactly one organization,
-- branch, cash register and cash shift. This migration intentionally models only
-- the OPEN -> CANCELLED lifecycle. Payment, completion, inventory and receipts
-- are separate future boundaries.
--
-- V1-V12 are published and immutable; this migration is additive.

-- Superkey needed to bind a sale to a shift in the exact same operational scope.
ALTER TABLE cash_shifts
    ADD CONSTRAINT cash_shifts_id_scope_unique UNIQUE (id, organization_id, branch_id, cash_register_id);

-- V10 already created the group same-organization superkey. Options need this
-- supporting superkey for immutable sale-item modifier snapshots.
ALTER TABLE modifier_options
    ADD CONSTRAINT modifier_options_id_organization_unique UNIQUE (id, organization_id);

CREATE TABLE sales (
    id                   UUID          NOT NULL PRIMARY KEY,
    organization_id      UUID          NOT NULL REFERENCES organizations (id),
    branch_id            UUID          NOT NULL,
    cash_register_id     UUID          NOT NULL,
    cash_shift_id        UUID          NOT NULL,
    created_by_user_id   UUID          NOT NULL,
    cancelled_by_user_id UUID          NULL,
    status               VARCHAR(16)   NOT NULL CHECK (status IN ('OPEN', 'CANCELLED')),
    total_amount         NUMERIC(19,4) NOT NULL CHECK (total_amount >= 0),
    version              BIGINT        NOT NULL DEFAULT 0,
    created_at           TIMESTAMPTZ   NOT NULL,
    updated_at           TIMESTAMPTZ   NOT NULL,
    cancelled_at         TIMESTAMPTZ   NULL,
    CONSTRAINT sales_cash_shift_scope_fk
        FOREIGN KEY (cash_shift_id, organization_id, branch_id, cash_register_id)
        REFERENCES cash_shifts (id, organization_id, branch_id, cash_register_id),
    CONSTRAINT sales_created_by_fk
        FOREIGN KEY (created_by_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT sales_cancelled_by_fk
        FOREIGN KEY (cancelled_by_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT sales_state_consistency CHECK (
        (status = 'OPEN' AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL)
        OR
        (status = 'CANCELLED' AND cancelled_at IS NOT NULL AND cancelled_by_user_id IS NOT NULL)
    ),
    CONSTRAINT sales_id_organization_unique UNIQUE (id, organization_id)
);

CREATE TABLE sale_items (
    id                    UUID          NOT NULL PRIMARY KEY,
    organization_id       UUID          NOT NULL,
    sale_id               UUID          NOT NULL,
    product_id            UUID          NOT NULL,
    product_sku_snapshot  VARCHAR(64)   NOT NULL,
    product_name_snapshot VARCHAR(120)  NOT NULL CHECK (length(trim(product_name_snapshot)) > 0),
    base_unit_price       NUMERIC(19,4) NOT NULL CHECK (base_unit_price >= 0),
    modifier_unit_delta   NUMERIC(19,4) NOT NULL,
    effective_unit_price  NUMERIC(19,4) NOT NULL CHECK (effective_unit_price >= 0),
    quantity              INTEGER       NOT NULL CHECK (quantity >= 1 AND quantity <= 1000000),
    line_total            NUMERIC(19,4) NOT NULL CHECK (line_total >= 0),
    status                VARCHAR(16)   NOT NULL CHECK (status IN ('ACTIVE', 'REMOVED')),
    position              INTEGER       NOT NULL CHECK (position >= 0),
    created_at            TIMESTAMPTZ   NOT NULL,
    updated_at            TIMESTAMPTZ   NOT NULL,
    removed_at            TIMESTAMPTZ   NULL,
    removed_by_user_id    UUID          NULL,
    CONSTRAINT sale_items_sale_scope_fk
        FOREIGN KEY (sale_id, organization_id) REFERENCES sales (id, organization_id),
    CONSTRAINT sale_items_product_scope_fk
        FOREIGN KEY (product_id, organization_id) REFERENCES products (id, organization_id),
    CONSTRAINT sale_items_removed_by_fk
        FOREIGN KEY (removed_by_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT sale_items_state_consistency CHECK (
        (status = 'ACTIVE' AND removed_at IS NULL AND removed_by_user_id IS NULL)
        OR
        (status = 'REMOVED' AND removed_at IS NOT NULL AND removed_by_user_id IS NOT NULL)
    ),
    CONSTRAINT sale_items_sale_position_unique UNIQUE (sale_id, position),
    CONSTRAINT sale_items_id_organization_unique UNIQUE (id, organization_id)
);

CREATE TABLE sale_item_modifiers (
    id                    UUID          NOT NULL PRIMARY KEY,
    organization_id       UUID          NOT NULL,
    sale_item_id          UUID          NOT NULL,
    modifier_group_id     UUID          NOT NULL,
    modifier_option_id    UUID          NOT NULL,
    group_code_snapshot   VARCHAR(64)   NOT NULL,
    group_name_snapshot   VARCHAR(120)  NOT NULL CHECK (length(trim(group_name_snapshot)) > 0),
    option_code_snapshot  VARCHAR(64)   NOT NULL,
    option_name_snapshot  VARCHAR(120)  NOT NULL CHECK (length(trim(option_name_snapshot)) > 0),
    price_delta_snapshot  NUMERIC(19,4) NOT NULL,
    display_order         INTEGER       NOT NULL CHECK (display_order >= 0),
    created_at            TIMESTAMPTZ   NOT NULL,
    CONSTRAINT sale_item_modifiers_item_scope_fk
        FOREIGN KEY (sale_item_id, organization_id) REFERENCES sale_items (id, organization_id),
    CONSTRAINT sale_item_modifiers_group_scope_fk
        FOREIGN KEY (modifier_group_id, organization_id) REFERENCES modifier_groups (id, organization_id),
    CONSTRAINT sale_item_modifiers_option_org_fk
        FOREIGN KEY (modifier_option_id, organization_id) REFERENCES modifier_options (id, organization_id),
    CONSTRAINT sale_item_modifiers_item_option_unique UNIQUE (sale_item_id, modifier_option_id)
);

-- Read current/history sales for a shift and recalculate an aggregate from its
-- active lines. The child indexes also support mapped detail reads.
CREATE INDEX idx_sales_shift_status_created ON sales (cash_shift_id, status, created_at DESC, id DESC);
CREATE INDEX idx_sale_items_sale_status_position ON sale_items (sale_id, status, position);
CREATE INDEX idx_sale_item_modifiers_item_order ON sale_item_modifiers (sale_item_id, display_order);

-- Existing roles keep their grants: V13 adds catalog entries only. Fresh bootstrap
-- grants every catalog permission through the existing bootstrap query.
INSERT INTO permissions (code, description) VALUES
    ('sale.read',   'List and read sales of a cash shift'),
    ('sale.create', 'Create an OPEN sale on a cash shift'),
    ('sale.update', 'Add, update and remove items of an OPEN sale'),
    ('sale.cancel', 'Cancel an OPEN sale for operational recovery');

COMMENT ON TABLE sales IS
    'Operational transaction under construction. Lifecycle is OPEN -> CANCELLED only; no payment, completion, inventory, tax, discount or receipt semantics exist in this foundation.';
COMMENT ON TABLE sale_items IS
    'Historical line snapshots for a sale. Removed lines are retained and excluded from sales.total_amount; product catalog mutations never rewrite snapshots.';
COMMENT ON TABLE sale_item_modifiers IS
    'Historical modifier snapshots for a sale item. Options are server-resolved from the product configuration; client-supplied names or prices are never accepted.';
