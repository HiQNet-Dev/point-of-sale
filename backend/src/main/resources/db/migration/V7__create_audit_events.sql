-- Audit Foundation: permanent, append-only history of administrative and
-- security-relevant mutations of the POS.
--
-- Audit_events is NOT an application/security/debug log: it is persistent
-- business-administration history. APPEND-ONLY is a SECURITY INVARIANT
-- enforced at the database level: a BEFORE UPDATE OR DELETE trigger rejects
-- every attempt to modify or remove an existing event. The application exposes
-- no update/delete API and its repository exposes no delete capability either;
-- the trigger is the final barrier and does not depend on JPA/Java discipline.
--
-- The database owner (migration authority) can always alter the schema on
-- purpose; the ordinary application cannot mutate existing events.

CREATE TABLE audit_events (
    id              UUID         NOT NULL PRIMARY KEY,
    organization_id UUID         NOT NULL REFERENCES organizations (id),
    branch_id       UUID,
    actor_type      VARCHAR(16)  NOT NULL CHECK (actor_type IN ('USER', 'SYSTEM')),
    actor_user_id   UUID,
    token_jti       VARCHAR(64),
    action          VARCHAR(80)  NOT NULL CHECK (length(trim(action)) > 0),
    entity_type     VARCHAR(40)  NOT NULL CHECK (length(trim(entity_type)) > 0),
    entity_id       UUID,
    reason          VARCHAR(255),
    metadata        JSONB,
    occurred_at     TIMESTAMPTZ  NOT NULL,
    -- Actor semantics: a USER event carries the acting identity; a SYSTEM
    -- event (internal/bootstrap/maintenance process) never does.
    CONSTRAINT audit_events_actor_check CHECK (
        (actor_type = 'USER' AND actor_user_id IS NOT NULL)
        OR (actor_type = 'SYSTEM' AND actor_user_id IS NULL)
    ),
    -- Metadata is small structured context, not arbitrary storage: bounded at
    -- 16 KiB of serialized JSON (the application enforces this too).
    CONSTRAINT audit_events_metadata_size CHECK (
        metadata IS NULL OR octet_length(metadata::text) <= 16384
    ),
    -- Tenant isolation: an audit event belongs to EXACTLY one organization;
    -- an optional branch must belong to the same organization (composite FK
    -- over the V6 superkey branches(id, organization_id)).
    CONSTRAINT audit_events_branch_fk
        FOREIGN KEY (branch_id, organization_id) REFERENCES branches (id, organization_id),
    -- Actor isolation: a USER actor must belong to the same organization
    -- (composite FK over the V5 superkey users(id, organization_id)). An event
    -- Organization A with an acting User of Organization B is impossible.
    CONSTRAINT audit_events_actor_fk
        FOREIGN KEY (actor_user_id, organization_id) REFERENCES users (id, organization_id)
);

-- Query-driven indexes (the read API always filters by organization and orders
-- by occurred_at DESC, id DESC):
--   (organization_id, occurred_at DESC, id DESC): the main listing.
--   (organization_id, branch_id, occurred_at DESC): the branchId filter.
--   (organization_id, actor_user_id, occurred_at DESC): the actorUserId filter.
--   (organization_id, entity_type, entity_id, occurred_at DESC): the
--       entityType/entityId filter.
CREATE INDEX idx_audit_events_org_occurred
    ON audit_events (organization_id, occurred_at DESC, id DESC);
CREATE INDEX idx_audit_events_org_branch_occurred
    ON audit_events (organization_id, branch_id, occurred_at DESC);
CREATE INDEX idx_audit_events_org_actor_occurred
    ON audit_events (organization_id, actor_user_id, occurred_at DESC);
CREATE INDEX idx_audit_events_org_entity_occurred
    ON audit_events (organization_id, entity_type, entity_id, occurred_at DESC);

-- Append-only enforcement.
CREATE OR REPLACE FUNCTION audit_events_reject_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit_events is append-only: % is not allowed', TG_OP
        USING ERRCODE = 'check_violation';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_events_append_only
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION audit_events_reject_mutation();

-- Permission catalog addition (idempotent single insert; catalog: 18 -> 19).
INSERT INTO permissions (code, description) VALUES
    ('audit.read', 'Read the organization audit log');

COMMENT ON TABLE audit_events IS
    'Append-only administrative/security audit history. UPDATE/DELETE are rejected by a BEFORE trigger (audit immutability invariant); retention/archival policy is pending by design (no purge yet).';
COMMENT ON COLUMN audit_events.token_jti IS
    'Correlation id of the access JWT used by the actor (claim jti). Never stores the JWT itself, an Authorization header or any token hash.';