-- Branch Memberships: explicit user -> branch link with branch-scoped roles.
--
-- Two authorization scopes coexist:
--   ORGANIZATION scope: user_organization_roles grant permissions org-wide.
--   BRANCH scope:       a BranchMembership (ACTIVE) with its roles grants
--                        permissions ONLY for that branch (and only while the
--                        branch itself is ACTIVE).
--
-- A branch-scoped role NEVER promotes to organization scope. Management of
-- memberships (and their roles) is always authorized with organization-wide
-- permissions (branch.membership.*), never branch-scoped ones.
--
-- Tenant enforcement is database-level: the membership can only reference a
-- branch/user of the SAME organization and its roles can only reference roles
-- of the SAME organization. Composite FKs require the (id, organization_id)
-- superkey on branches (users/roles already have it from V5).

ALTER TABLE branches
    ADD CONSTRAINT branches_id_organization_unique UNIQUE (id, organization_id);

CREATE TABLE branch_memberships (
    id              UUID        NOT NULL PRIMARY KEY,
    organization_id UUID        NOT NULL,
    branch_id       UUID        NOT NULL,
    user_id         UUID        NOT NULL,
    status          VARCHAR(16) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT      NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL,
    CONSTRAINT branch_memberships_org_branch_user_unique
        UNIQUE (organization_id, branch_id, user_id),
    CONSTRAINT branch_memberships_id_organization_unique
        UNIQUE (id, organization_id),
    CONSTRAINT branch_memberships_branch_fk
        FOREIGN KEY (branch_id, organization_id) REFERENCES branches (id, organization_id),
    CONSTRAINT branch_memberships_user_fk
        FOREIGN KEY (user_id, organization_id) REFERENCES users (id, organization_id)
);

CREATE TABLE branch_membership_roles (
    organization_id UUID        NOT NULL,
    membership_id   UUID        NOT NULL,
    role_id         UUID        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    CONSTRAINT branch_membership_roles_pk PRIMARY KEY (organization_id, membership_id, role_id),
    CONSTRAINT branch_membership_roles_membership_fk
        FOREIGN KEY (membership_id, organization_id) REFERENCES branch_memberships (id, organization_id),
    CONSTRAINT branch_membership_roles_role_fk
        FOREIGN KEY (role_id, organization_id) REFERENCES roles (id, organization_id)
);

-- Reads: "list memberships of a branch" and "user's memberships across org".
CREATE INDEX idx_branch_memberships_org_branch ON branch_memberships (organization_id, branch_id);
CREATE INDEX idx_branch_memberships_org_user ON branch_memberships (organization_id, user_id);
CREATE INDEX idx_branch_membership_roles_membership ON branch_membership_roles (organization_id, membership_id);

-- Permission catalog additions (idempotent single insert; existing 14 codes
-- stay untouched. Catalog: 14 -> 18).
INSERT INTO permissions (code, description) VALUES
    ('branch.membership.read',         'Read branch memberships'),
    ('branch.membership.create',       'Create branch memberships'),
    ('branch.membership.update',       'Update branch memberships'),
    ('branch.membership.roles.manage', 'Replace a membership''s branch role set');

COMMENT ON TABLE branch_memberships IS
    'Explicit link: a user belongs to a branch (at most one membership per organization+branch+user). status INACTIVE grants no branch permission while keeping the configuration and its roles.';
COMMENT ON TABLE branch_membership_roles IS
    'Branch-scoped role assignments of a membership. Roles are the organization-scoped role definitions (no separate BranchRole model). Composite FKs enforce the same organization on membership and role (no cross-tenant assignment).';