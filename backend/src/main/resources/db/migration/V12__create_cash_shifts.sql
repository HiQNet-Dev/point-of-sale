-- Cash Shifts Foundation: operational shift (session) of a branch-owned cash
-- register (CashRegister -> CashShift).
--
-- V1-V11 are published and immutable; this migration is additive.
--
-- A CashShift is a HISTORICAL operational record: a register accumulates an
-- unlimited CLOSED history and may have AT MOST ONE OPEN shift at any time.
-- The one-open-per-register rule is enforced by the DATABASE (a partial unique
-- index), never by application code alone: a concurrent double-open is resolved
-- by PostgreSQL, not by a JVM check.
--
-- Explicitly out of scope here: sales, sale items, payments, tickets, cash
-- movements (deposits/withdrawals/refunds), expected cash, reconciliations,
-- variance, drawers, printers, terminals and any hardware. Because none of
-- those exist yet, closingDeclaredAmount is a DECLARATION by the operator, not
-- a computed reconciliation result.
--
-- Tenant + scope integrity is database-level. The shift can only reference a
-- register that exists in the SAME organization AND branch (composite FK over a
-- new (id, organization_id, branch_id) superkey on cash_registers), and its
-- opened/closed actors must belong to the SAME organization (composite FK over
-- the existing users (id, organization_id) superkey). Java validation is NOT the
-- only barrier.

-- Supporting superkey required for the composite register FK below. id is
-- already the PK; this composite unique is redundant-but-required (same
-- technique already used by V5/V6 for users/roles/branches).
ALTER TABLE cash_registers
    ADD CONSTRAINT cash_registers_id_scope_unique UNIQUE (id, organization_id, branch_id);

CREATE TABLE cash_shifts (
    id                      UUID          NOT NULL PRIMARY KEY,
    organization_id         UUID          NOT NULL,
    branch_id               UUID          NOT NULL,
    cash_register_id        UUID          NOT NULL,
    -- Actors are server-derived from the authenticated principal, never sent by
    -- the client. openedBy is required; closedBy is NULL while the shift is OPEN.
    opened_by_user_id       UUID          NOT NULL,
    closed_by_user_id       UUID          NULL,
    status                  VARCHAR(16)   NOT NULL CHECK (status IN ('OPEN', 'CLOSED')),
    -- Money is NUMERIC(19,4): never double/float. Both amounts are declared by
    -- the operator (cash physically handed over), not computed.
    opening_amount          NUMERIC(19,4) NOT NULL,
    closing_declared_amount NUMERIC(19,4) NULL,
    opened_at               TIMESTAMPTZ   NOT NULL,
    closed_at               TIMESTAMPTZ   NULL,
    version                 BIGINT        NOT NULL DEFAULT 0,
    created_at              TIMESTAMPTZ   NOT NULL,
    updated_at              TIMESTAMPTZ   NOT NULL,
    -- The register must be an ACTUAL register of the SAME organization AND the
    -- SAME branch (organization_id + branch_id must match the register row).
    CONSTRAINT cash_shifts_register_fk
        FOREIGN KEY (cash_register_id, organization_id, branch_id)
        REFERENCES cash_registers (id, organization_id, branch_id),
    -- openedBy must belong to the same organization (cross-tenant impossible).
    CONSTRAINT cash_shifts_opened_by_fk
        FOREIGN KEY (opened_by_user_id, organization_id) REFERENCES users (id, organization_id),
    -- closedBy is nullable; when set it must belong to the same organization.
    CONSTRAINT cash_shifts_closed_by_fk
        FOREIGN KEY (closed_by_user_id, organization_id) REFERENCES users (id, organization_id),
    -- No partially closed states: OPEN has no closing data, CLOSED has all of it.
    CONSTRAINT cash_shifts_state_consistency CHECK (
        (status = 'OPEN'   AND closed_at IS NULL     AND closed_by_user_id IS NULL     AND closing_declared_amount IS NULL)
        OR
        (status = 'CLOSED' AND closed_at IS NOT NULL AND closed_by_user_id IS NOT NULL AND closing_declared_amount IS NOT NULL)
    ),
    CONSTRAINT cash_shifts_opening_amount_non_negative CHECK (opening_amount >= 0),
    CONSTRAINT cash_shifts_closing_declared_amount_non_negative CHECK (closing_declared_amount >= 0)
);

-- CRITICAL INVARIANT: at most ONE OPEN shift per cash register. A partial unique
-- index is the final authority; a concurrent second open is rejected by
-- PostgreSQL (mapped to 409 CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT), never by a
-- racy SELECT-then-INSERT in the JVM. CLOSED rows are unconstrained: the history
-- is unlimited.
CREATE UNIQUE INDEX cash_shifts_one_open_per_register
    ON cash_shifts (cash_register_id)
    WHERE status = 'OPEN';

-- Reads: "list a register's shifts" ordered by most recent first (opened_at
-- DESC, id DESC). Also serves scoped lookups that filter by register.
CREATE INDEX idx_cash_shifts_register_opened ON cash_shifts (cash_register_id, opened_at DESC, id DESC);

-- Permission catalog additions (idempotent single insert; existing 35 codes stay
-- untouched. Cash shifts: 35 -> 38). No cash-shift.update/delete/reopen: a CLOSED
-- shift is historical and immutable.
INSERT INTO permissions (code, description) VALUES
    ('cash-shift.read',  'List and read cash shifts of a cash register'),
    ('cash-shift.open',  'Open a cash shift on a cash register'),
    ('cash-shift.close', 'Close an open cash shift');

COMMENT ON TABLE cash_shifts IS
    'Operational shift (session) of a branch-owned cash register. At most ONE OPEN shift per cash register (DB partial unique index); unlimited CLOSED history. No hard delete, no reopen: CLOSED is historical. opening/closing amounts are operator declarations, NOT reconciliations (no sales/payments/cash movements exist yet).';
COMMENT ON COLUMN cash_shifts.opening_amount IS
    'Cash declared by the operator when the shift opens. NUMERIC(19,4), >= 0. Currency is Organization.currency (no per-row currency column).';
COMMENT ON COLUMN cash_shifts.closing_declared_amount IS
    'Cash physically declared by the operator when the shift closes. NULL while OPEN; NOT a computed expected balance (no sales/payments/cash movements yet). NUMERIC(19,4), >= 0.';
COMMENT ON COLUMN cash_shifts.cash_register_id IS
    'Register this shift belongs to; composite FK (cash_register_id, organization_id, branch_id) keeps it in the same organization AND branch.';
COMMENT ON INDEX cash_shifts_one_open_per_register IS
    'Enforces at most one OPEN shift per cash register (the authorization authority for concurrent opens).';
