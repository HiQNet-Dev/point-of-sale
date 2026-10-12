-- Organization-owned Inventory Item definitions. Physical stock, movements,
-- recipes and Sales integration are deliberately introduced by later slices.
-- Existing role grants are not changed by adding permission catalog entries.

CREATE TABLE inventory_items (
    id                       UUID         NOT NULL PRIMARY KEY,
    organization_id          UUID         NOT NULL REFERENCES organizations (id),
    code                     VARCHAR(64)  NOT NULL,
    name                     VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    description              VARCHAR(1000),
    base_unit                VARCHAR(16)  NOT NULL CHECK (base_unit IN ('UNIT', 'GRAM', 'MILLILITER')),
    fractional_unit_allowed  BOOLEAN      NOT NULL DEFAULT false,
    status                   VARCHAR(16)  NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version                  BIGINT       NOT NULL DEFAULT 0,
    created_at               TIMESTAMPTZ  NOT NULL,
    updated_at               TIMESTAMPTZ  NOT NULL,
    CONSTRAINT inventory_items_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT inventory_items_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    CONSTRAINT inventory_items_description_not_blank CHECK (
        description IS NULL OR length(trim(description)) > 0
    ),
    CONSTRAINT inventory_items_id_organization_unique UNIQUE (id, organization_id)
);

CREATE INDEX idx_inventory_items_organization_name
    ON inventory_items (organization_id, name);

INSERT INTO permissions (code, description) VALUES
    ('inventory.item.read',  'List and read organization inventory item definitions'),
    ('inventory.item.write', 'Create and update organization inventory item definitions');

COMMENT ON TABLE inventory_items IS
    'Organization-owned inventory item definition. Branch stock, movements, recipes and quantity balances are introduced by later migrations.';
COMMENT ON COLUMN inventory_items.base_unit IS
    'Immutable after recipes, branch balances or movements reference the item; V16 has no such dependent data yet.';
COMMENT ON COLUMN inventory_items.fractional_unit_allowed IS
    'Metadata for future quantity validation; false by default. This migration does not create or validate stock quantities.';
