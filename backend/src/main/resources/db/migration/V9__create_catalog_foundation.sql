-- Catalog Foundation: organization-owned Category and Product.
--
-- V1-V8 are published and immutable; this migration is additive.
--
-- Ownership: the catalog belongs to the ORGANIZATION, not the branch.
-- (catalog = organization, operation = branch). Branches consume it later;
-- branch availability and branch pricing are explicitly out of scope here.
--
-- No hard delete: a category/product is retired by status INACTIVE while kept
-- in the database. status is VARCHAR + CHECK (no native PG enum), consistent
-- with the rest of the schema.
--
-- Tenant enforcement is database-level: a product can only reference a
-- category of the SAME organization. This is enforced with a composite FK over
-- the categories(id, organization_id) superkey; Java validation is NOT the only
-- barrier.

CREATE TABLE categories (
    id              UUID         NOT NULL PRIMARY KEY,
    organization_id UUID         NOT NULL REFERENCES organizations (id),
    code            VARCHAR(64)  NOT NULL,
    name            VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    status          VARCHAR(16)  NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT       NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ  NOT NULL,
    updated_at      TIMESTAMPTZ  NOT NULL,
    -- code is a normalized slug, unique per organization and immutable.
    CONSTRAINT categories_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT categories_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    -- Superkey enabler for the products -> categories same-organization FK.
    CONSTRAINT categories_id_organization_unique UNIQUE (id, organization_id)
);

CREATE TABLE products (
    id              UUID          NOT NULL PRIMARY KEY,
    organization_id UUID          NOT NULL REFERENCES organizations (id),
    category_id     UUID,
    -- sku is the business identifier: trimmed, uppercase canonical form,
    -- immutable, unique per organization. It is NOT the UUID.
    sku             VARCHAR(64)   NOT NULL,
    -- barcode is optional; kept as text to preserve leading zeros.
    barcode         VARCHAR(64),
    name            VARCHAR(120)  NOT NULL CHECK (length(trim(name)) > 0),
    description     VARCHAR(1000),
    -- Organization base price; currency is inherited from Organization.currency.
    unit_price      NUMERIC(19,4) NOT NULL CHECK (unit_price >= 0),
    status          VARCHAR(16)   NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT        NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ   NOT NULL,
    updated_at      TIMESTAMPTZ   NOT NULL,
    CONSTRAINT products_organization_sku_unique UNIQUE (organization_id, sku),
    CONSTRAINT products_sku_format CHECK (sku ~ '^[A-Z0-9._-]{1,64}$'),
    CONSTRAINT products_description_not_blank CHECK (
        description IS NULL OR length(trim(description)) > 0
    ),
    -- Same-organization category: when category_id is NULL the FK is not
    -- enforced (MATCH SIMPLE); when set, the category must belong to the same
    -- organization as the product.
    CONSTRAINT products_category_fk
        FOREIGN KEY (category_id, organization_id) REFERENCES categories (id, organization_id)
);

-- Optional barcode: unique per organization ONLY when present (partial unique
-- index). Race-safe at the database level, not just a service precheck.
CREATE UNIQUE INDEX products_organization_barcode_unique
    ON products (organization_id, barcode)
    WHERE barcode IS NOT NULL;

-- Reads: category listing ordered by name; product listing ordered by name and
-- filtered by category.
CREATE INDEX idx_categories_organization_name ON categories (organization_id, name);
CREATE INDEX idx_products_organization_name ON products (organization_id, name);
CREATE INDEX idx_products_organization_category ON products (organization_id, category_id);

-- Permission catalog additions (idempotent single insert; existing 19 codes
-- stay untouched. Catalog: 19 -> 25).
INSERT INTO permissions (code, description) VALUES
    ('category.read',   'List and read categories'),
    ('category.create', 'Create categories'),
    ('category.update', 'Update categories'),
    ('product.read',    'List and read products'),
    ('product.create',  'Create products'),
    ('product.update',  'Update products');

COMMENT ON TABLE categories IS
    'Organization-owned product category. code is a normalized slug, unique per organization and immutable. status INACTIVE keeps the row and its products; it does not cascade to product status.';
COMMENT ON TABLE products IS
    'Organization-owned product with a base unit price. sku is the immutable business identifier (unique per organization); barcode is optional and unique per organization when present. No hard delete: retire via status INACTIVE.';
COMMENT ON COLUMN products.unit_price IS
    'Organization base price, NUMERIC(19,4). Currency is inherited from Organization.currency, never stored per product. Branch price overrides, promotions and taxes are out of scope.';
COMMENT ON COLUMN products.barcode IS
    'Optional barcode kept as text to preserve leading zeros. Unique per organization when present (partial unique index). No GS1 parsing or multi-barcode support yet.';
