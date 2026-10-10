-- Cash Registers Foundation: branch-owned CashRegister.
--
-- V1-V10 are published and immutable; this migration is additive.
--
-- A cash register is a physical/logical point of sale INSIDE a branch
-- (Branch -> CashRegister): it belongs to the BRANCH, not the organization,
-- unlike the catalog/modifiers (catalog = organization, operation = branch).
-- Explicitly out of scope here: shifts, sales, payments, tickets, balances,
-- opening/closing amounts, drawers, printers, terminals and any hardware.
--
-- No hard delete: a register is retired via status INACTIVE while kept in the
-- database, and deactivation never cascades to any other row. status is
-- VARCHAR + CHECK (no native PG enum), consistent with the rest of the schema.
--
-- Tenant + branch integrity is database-level: a register can only reference an
-- ACTUAL branch of the SAME organization. This is enforced with a composite FK
-- over the (id, organization_id) superkey on branches (created in V6); Java
-- validation is NOT the only barrier.

CREATE TABLE cash_registers (
    id              UUID         NOT NULL PRIMARY KEY,
    organization_id UUID         NOT NULL REFERENCES organizations (id),
    branch_id       UUID         NOT NULL,
    -- code is a normalized slug, unique per branch and immutable.
    code            VARCHAR(64)  NOT NULL,
    name            VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    status          VARCHAR(16)  NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT       NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ  NOT NULL,
    updated_at      TIMESTAMPTZ  NOT NULL,
    -- code identifies a register within its branch (two branches may reuse it).
    CONSTRAINT cash_registers_branch_code_unique UNIQUE (branch_id, code),
    CONSTRAINT cash_registers_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    -- The register's organization must match its branch's organization.
    CONSTRAINT cash_registers_branch_fk
        FOREIGN KEY (branch_id, organization_id) REFERENCES branches (id, organization_id)
);

-- Reads: "list cash registers of a branch" ordered by name. The composite unique
-- (branch_id, code) already covers branch_id-prefixed lookups by code.
CREATE INDEX idx_cash_registers_branch_name ON cash_registers (branch_id, name);

-- Permission catalog additions (idempotent single insert; existing 32 codes stay
-- untouched. Cash registers: 32 -> 35).
INSERT INTO permissions (code, description) VALUES
    ('cash-register.read',   'List and read cash registers of a branch'),
    ('cash-register.create', 'Create cash registers in a branch'),
    ('cash-register.update', 'Update cash registers (name, status)');

COMMENT ON TABLE cash_registers IS
    'Branch-owned point of sale inside a branch (Branch -> CashRegister). code is a normalized slug, unique per branch and immutable. status INACTIVE retires the register while keeping its configuration; there is no hard delete. No shift/sale/balance state is modeled here.';
COMMENT ON COLUMN cash_registers.code IS
    'Normalized lowercase slug, immutable, unique within the branch (two branches may reuse the same code). Race-safe uniqueness is enforced by cash_registers_branch_code_unique.';
COMMENT ON COLUMN cash_registers.status IS
    'ACTIVE | INACTIVE. Deactivating a register removes nothing and never modifies its branch or any other resource.';
