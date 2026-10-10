-- Product Modifiers / Options Foundation: organization-owned ModifierGroup and
-- ModifierOption, plus the ordered product <-> modifier-group configuration link.
--
-- V1-V9 are published and immutable; this migration is additive.
--
-- Modifiers are OPTIONS, not independent sellable products: a modifier group is
-- a reusable, organization-owned catalog of choices (e.g. "Milk", "Size") with
-- its own options and a SIGNED price delta (+, 0 or -). It is NOT a
-- ProductVariant and does NOT split a product's SKU/barcode/stock; variants,
-- branch pricing, taxes and inventory are explicitly out of scope here.
--
-- Ownership: modifiers belong to the ORGANIZATION, not the branch
-- (modifier = organization, operation = branch), exactly like the catalog.
--
-- No hard delete: a group/option is retired via status INACTIVE while kept in the
-- database, and deactivation never cascades to another row. status is VARCHAR +
-- CHECK (no native PG enum), consistent with the rest of the schema.
--
-- Selection semantics are DERIVED, never stored: required = min_selections > 0,
-- single = max_selections = 1, multi = max_selections > 1.
--
-- The product references modifier groups through product_modifier_groups: an
-- ordered configuration list whose concurrency boundary is the product's own
-- version column (no separate configuration-version column).
--
-- Tenant enforcement is database-level: an option can only reference a group of
-- the SAME organization and a product can only reference a group of the SAME
-- organization. This is enforced with composite FKs over the
-- (id, organization_id) superkeys; Java validation is NOT the only barrier.

-- Superkey enabler for the product_modifier_groups -> products same-organization
-- FK. V9 created products(id) as the primary key; add the composite superkey.
ALTER TABLE products
    ADD CONSTRAINT products_id_organization_unique UNIQUE (id, organization_id);

CREATE TABLE modifier_groups (
    id              UUID         NOT NULL PRIMARY KEY,
    organization_id UUID         NOT NULL REFERENCES organizations (id),
    code            VARCHAR(64)  NOT NULL,
    name            VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    min_selections  INTEGER      NOT NULL DEFAULT 0 CHECK (min_selections >= 0),
    max_selections  INTEGER      NOT NULL DEFAULT 1 CHECK (max_selections >= 1),
    status          VARCHAR(16)  NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT       NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ  NOT NULL,
    updated_at      TIMESTAMPTZ  NOT NULL,
    -- code is a normalized slug, unique per organization and immutable.
    CONSTRAINT modifier_groups_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT modifier_groups_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    CONSTRAINT modifier_groups_selection_bounds CHECK (min_selections <= max_selections),
    -- Superkey enabler for the modifier_options -> modifier_groups same-org FK
    -- and the product_modifier_groups -> modifier_groups same-org FK.
    CONSTRAINT modifier_groups_id_organization_unique UNIQUE (id, organization_id)
);

CREATE TABLE modifier_options (
    id                UUID          NOT NULL PRIMARY KEY,
    organization_id   UUID          NOT NULL,
    modifier_group_id UUID          NOT NULL,
    code              VARCHAR(64)   NOT NULL,
    name              VARCHAR(120)  NOT NULL CHECK (length(trim(name)) > 0),
    -- Signed delta applied on top of the product base price; may be negative
    -- (discount), zero or positive. Currency is inherited from the organization.
    price_delta       NUMERIC(19,4) NOT NULL DEFAULT 0,
    display_order     INTEGER       NOT NULL DEFAULT 0 CHECK (display_order >= 0),
    status            VARCHAR(16)   NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version           BIGINT        NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ   NOT NULL,
    updated_at        TIMESTAMPTZ   NOT NULL,
    -- code is a normalized slug, unique within the group and immutable.
    CONSTRAINT modifier_options_group_code_unique UNIQUE (modifier_group_id, code),
    CONSTRAINT modifier_options_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    -- Same-organization group: the option's organization must match its group's.
    CONSTRAINT modifier_options_group_fk
        FOREIGN KEY (modifier_group_id, organization_id) REFERENCES modifier_groups (id, organization_id)
);

CREATE TABLE product_modifier_groups (
    organization_id   UUID        NOT NULL,
    product_id        UUID        NOT NULL,
    modifier_group_id UUID        NOT NULL,
    display_order     INTEGER     NOT NULL DEFAULT 0 CHECK (display_order >= 0),
    created_at        TIMESTAMPTZ NOT NULL,
    -- A product references a modifier group at most once; the order of the list
    -- IS the display_order (position in the replacement request).
    CONSTRAINT product_modifier_groups_pk PRIMARY KEY (product_id, modifier_group_id),
    CONSTRAINT product_modifier_groups_product_fk
        FOREIGN KEY (product_id, organization_id) REFERENCES products (id, organization_id),
    CONSTRAINT product_modifier_groups_group_fk
        FOREIGN KEY (modifier_group_id, organization_id) REFERENCES modifier_groups (id, organization_id)
);

-- Reads: group listing ordered by name; option listing ordered by display_order;
-- product assignment listing ordered by display_order.
CREATE INDEX idx_modifier_groups_organization_name ON modifier_groups (organization_id, name);
CREATE INDEX idx_modifier_options_group_order ON modifier_options (modifier_group_id, display_order);
CREATE INDEX idx_product_modifier_groups_product_order
    ON product_modifier_groups (product_id, display_order);

-- Permission catalog additions (idempotent single insert; existing 25 codes stay
-- untouched. Modifiers: 25 -> 32).
INSERT INTO permissions (code, description) VALUES
    ('modifier.group.read',      'List and read modifier groups'),
    ('modifier.group.create',    'Create modifier groups'),
    ('modifier.group.update',    'Update modifier groups'),
    ('modifier.option.read',     'List and read modifier options'),
    ('modifier.option.create',   'Create modifier options'),
    ('modifier.option.update',   'Update modifier options'),
    ('product.modifiers.manage', 'Replace the modifier groups assigned to a product');

COMMENT ON TABLE modifier_groups IS
    'Organization-owned reusable modifier group (e.g. Milk, Size). code is a normalized slug, unique per organization and immutable. Selection semantics are derived: required = min_selections > 0, single = max_selections = 1, multi = max_selections > 1; they are never stored. status INACTIVE keeps the group, its options and its product assignments.';
COMMENT ON TABLE modifier_options IS
    'Organization-owned option of a modifier group (e.g. Whole, Skim). code is a normalized slug, unique within the group and immutable. price_delta is a SIGNED NUMERIC(19,4) added to the product base price (negative = discount). No hard delete: retire via status INACTIVE.';
COMMENT ON TABLE product_modifier_groups IS
    'Ordered configuration list: the modifier groups attached to a product. display_order is the position in the client-provided replacement list. Concurrency is governed by the product version column (no separate configuration version). Composite FKs keep product and group in the same organization.';
COMMENT ON COLUMN modifier_options.price_delta IS
    'Signed price delta applied on top of the product base price, NUMERIC(19,4). Negative values are discounts. Currency is inherited from Organization.currency; branch price overrides, promotions and taxes are out of scope.';
COMMENT ON COLUMN modifier_groups.max_selections IS
    'Maximum options selectable from the group (>= 1). Combined with min_selections it expresses required/single/multi WITHOUT storing those derivations.';
