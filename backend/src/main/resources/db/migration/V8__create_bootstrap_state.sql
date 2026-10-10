-- Secure Admin Bootstrap: technical, singleton installation state.
--
-- This table records ONLY whether/when the first-run provisioning completed
-- and which identities it created. It is NOT a credential store: the bootstrap
-- secret lives exclusively in the deployment configuration (BOOTSTRAP_SECRET),
-- never in the database. There is exactly ONE row (id = 1, enforced by CHECK).

CREATE TABLE installation_bootstrap (
    id                    SMALLINT    NOT NULL PRIMARY KEY,
    completed_at          TIMESTAMPTZ,
    organization_id       UUID,
    administrator_user_id UUID,
    administrator_role_id UUID,
    -- Singleton: this table can only ever hold the single installation row.
    CONSTRAINT installation_bootstrap_singleton_check CHECK (id = 1),
    -- Completion invariant: the row is either fully PENDING (all NULL) or
    -- fully COMPLETED (all set); a partial state is impossible.
    CONSTRAINT installation_bootstrap_completion_check CHECK (
        (completed_at IS NULL AND organization_id IS NULL
            AND administrator_user_id IS NULL AND administrator_role_id IS NULL)
        OR (completed_at IS NOT NULL AND organization_id IS NOT NULL
            AND administrator_user_id IS NOT NULL AND administrator_role_id IS NOT NULL)
    ),
    CONSTRAINT installation_bootstrap_organization_fk
        FOREIGN KEY (organization_id) REFERENCES organizations (id),
    -- Same-organization actors: the administrator user and role must belong to
    -- the recorded organization (composite FKs over the V5 superkeys
    -- users(id, organization_id) and roles(id, organization_id)).
    CONSTRAINT installation_bootstrap_admin_user_fk
        FOREIGN KEY (administrator_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT installation_bootstrap_admin_role_fk
        FOREIGN KEY (administrator_role_id, organization_id) REFERENCES roles (id, organization_id)
);

-- Initial PENDING state (every installation starts "not bootstrapped").
INSERT INTO installation_bootstrap (id) VALUES (1);

COMMENT ON TABLE installation_bootstrap IS
    'One-shot installation provisioning state. PENDING (all NULL) or COMPLETED (all set); the bootstrap secret is never stored here, only in deployment configuration.';