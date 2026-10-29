-- PROPUESTA D-AUTH-01/02/04/05. No ejecutar sin aceptación formal.
-- Depende de migrations/local/0001_organization_branch.sql.
-- SQLite STRICT. Los UUID y timestamps siguen el formato de 0001.

CREATE TABLE pos_user (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    display_name TEXT NOT NULL CHECK (
        instr(display_name,char(0))=0 AND length(trim(display_name)) > 0 AND length(display_name) <= 160
    ),
    email TEXT NOT NULL CHECK (
        instr(email,char(0))=0 AND length(email) BETWEEN 3 AND 254
        AND email = lower(trim(email)) AND instr(email,'@') > 1
    ),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (organization_id, email),
    -- Superclave para FKs compuestas same-org.
    UNIQUE (id, organization_id)
) STRICT;

CREATE TABLE local_credential (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    user_id TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    credential_kind TEXT NOT NULL CHECK (credential_kind IN ('PASSWORD','PIN')),
    algorithm TEXT NOT NULL CHECK (algorithm IN ('LEGACY_BCRYPT','ARGON2ID')),
    secret_hash TEXT NOT NULL CHECK (
        instr(secret_hash,char(0))=0 AND length(secret_hash) BETWEEN 1 AND 255
    ),
    params_json TEXT NOT NULL CHECK (
        json_valid(params_json) AND length(CAST(params_json AS BLOB)) <= 2048
    ),
    policy_id TEXT NOT NULL CHECK (
        length(policy_id) BETWEEN 1 AND 64
        AND policy_id NOT GLOB '*[^a-z0-9-]*'
    ),
    -- D-AUTH-02: catálogo vigente de F1-10 §16.5. La desactivación se modela en
    -- pos_user.status; no se añade DISABLED sin decisión formal.
    state TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (state IN ('ACTIVE','PASSWORD_RESET_REQUIRED')),
    credential_version INTEGER NOT NULL DEFAULT 1 CHECK (credential_version >= 1),
    failed_login_attempts INTEGER NOT NULL DEFAULT 0 CHECK (failed_login_attempts >= 0),
    locked_until TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (user_id, credential_kind),
    FOREIGN KEY (user_id, organization_id) REFERENCES pos_user(id, organization_id)
) STRICT;

CREATE TABLE local_session (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    user_id TEXT NOT NULL REFERENCES pos_user(id),
    session_hash TEXT NOT NULL UNIQUE CHECK (
        length(session_hash)=64 AND session_hash NOT GLOB '*[^0-9a-f]*'
    ),
    -- D-AUTH-03: columna física de F1-10 §16.45.
    state TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (state IN ('ACTIVE','REVOKED','EXPIRED')),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    session_version INTEGER NOT NULL DEFAULT 1 CHECK (session_version >= 1),
    CHECK (expires_at > created_at),
    CHECK (state <> 'REVOKED' OR revoked_at IS NOT NULL),
    CHECK (state <> 'ACTIVE' OR revoked_at IS NULL),
    -- Superclave para la FK de atribución de audit_event (sesión del actor).
    UNIQUE (id, user_id)
) STRICT;

CREATE INDEX idx_pos_user_organization ON pos_user (organization_id);
CREATE INDEX idx_local_credential_user ON local_credential (organization_id, user_id);
CREATE INDEX idx_local_session_user ON local_session (user_id, expires_at);
