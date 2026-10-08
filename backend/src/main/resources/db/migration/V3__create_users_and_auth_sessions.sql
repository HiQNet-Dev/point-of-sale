-- Authentication foundation: users (minimal identity for auth) and
-- refresh_sessions (opaque refresh token rotation/revocation).
-- The administrative CRUD for users arrives with the users module later.
-- status is VARCHAR + CHECK (same convention as V2, no native PG enum).

CREATE TABLE users (
    id                    UUID         NOT NULL PRIMARY KEY,
    organization_id       UUID         NOT NULL REFERENCES organizations (id),
    email                 VARCHAR(254) NOT NULL,
    password_hash         VARCHAR(60)  NOT NULL,
    status                VARCHAR(16)  NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    failed_login_attempts INTEGER      NOT NULL DEFAULT 0 CHECK (failed_login_attempts >= 0),
    locked_until          TIMESTAMPTZ,
    last_login_at         TIMESTAMPTZ,
    version               BIGINT       NOT NULL DEFAULT 0,
    created_at            TIMESTAMPTZ  NOT NULL,
    updated_at            TIMESTAMPTZ  NOT NULL,
    CONSTRAINT users_organization_email_unique UNIQUE (organization_id, email)
);

CREATE TABLE refresh_sessions (
    id                     UUID        NOT NULL PRIMARY KEY,
    user_id                UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash             VARCHAR(64) NOT NULL,
    family_id              UUID        NOT NULL,
    family_expires_at      TIMESTAMPTZ NOT NULL,
    created_at             TIMESTAMPTZ NOT NULL,
    expires_at             TIMESTAMPTZ NOT NULL,
    rotated_at             TIMESTAMPTZ,
    revoked_at             TIMESTAMPTZ,
    replaced_by_session_id UUID        REFERENCES refresh_sessions (id),
    CONSTRAINT refresh_sessions_token_hash_unique UNIQUE (token_hash)
);

-- Lookups: logout-all by user, reuse detection/rotation by token hash
-- (covered by the unique index), family revocation by family.
CREATE INDEX idx_refresh_sessions_user_id ON refresh_sessions (user_id);
CREATE INDEX idx_refresh_sessions_family_id ON refresh_sessions (family_id);

COMMENT ON TABLE users IS
    'Identity within an organization. email is unique per organization (not globally). lock is temporal via locked_until, never a permanent state.';
COMMENT ON TABLE refresh_sessions IS
    'Opaque refresh token sessions. Only SHA-256 hashes of tokens are stored; rotation marks the old session and creates a new one in the same family; presenting a rotated/revoked token revokes the whole family.';