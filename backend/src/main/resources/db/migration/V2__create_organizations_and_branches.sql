-- Organization + Branch foundation.
-- Multi-tenant SaaS is NOT implemented: organization_id scopes business data
-- within a single installation (Cliente A -> DB A -> Organization A).
-- status is VARCHAR + CHECK (no native PG enum: less rigidity, same safety).

CREATE TABLE organizations (
    id         UUID        NOT NULL PRIMARY KEY,
    name       VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    code       VARCHAR(64) NOT NULL,
    timezone   VARCHAR(64) NOT NULL,
    currency   VARCHAR(3) NOT NULL CHECK (char_length(currency) = 3),
    locale     VARCHAR(20) NOT NULL,
    status     VARCHAR(16) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version    BIGINT      NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT organizations_code_unique UNIQUE (code),
    CONSTRAINT organizations_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$')
);

CREATE TABLE branches (
    id              UUID        NOT NULL PRIMARY KEY,
    organization_id UUID        NOT NULL REFERENCES organizations (id),
    name            VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    code            VARCHAR(64) NOT NULL,
    timezone        VARCHAR(64),
    status          VARCHAR(16) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT      NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL,
    CONSTRAINT branches_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT branches_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$')
);

-- Speed up "list branches of organization" reads; the unique constraint
-- already covers (organization_id, code) lookups.
CREATE INDEX idx_branches_organization_id ON branches (organization_id);

COMMENT ON TABLE organizations IS
    'Root of configuration: owns branches and, later, users and catalog.';
COMMENT ON TABLE branches IS
    'Operational boundary: owns cash registers and inventory. timezone NULL means inherit from organization.';