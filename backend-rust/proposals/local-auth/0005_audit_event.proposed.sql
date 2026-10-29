-- PROPUESTA D-AUTH-07/08. No ejecutar sin aceptación formal.
-- Depende de 0001 (organization, branch), 0002 (pos_user, local_session).

CREATE TABLE audit_event (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('USER','SYSTEM')),
    actor_user_id TEXT,
    -- Nullable en el baseline; reemplaza conceptualmente al token_jti Java y
    -- nunca almacena el JWT ni un hash de token.
    session_id TEXT,
    action TEXT NOT NULL CHECK (length(trim(action)) > 0 AND length(action) <= 80),
    entity_type TEXT NOT NULL CHECK (length(trim(entity_type)) > 0 AND length(entity_type) <= 40),
    entity_id TEXT,
    reason TEXT CHECK (reason IS NULL OR length(reason) <= 255),
    metadata_json TEXT CHECK (
        metadata_json IS NULL
        OR (json_valid(metadata_json) AND length(CAST(metadata_json AS BLOB)) <= 16384)
    ),
    occurred_at TEXT NOT NULL CHECK (occurred_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
    event_version INTEGER NOT NULL DEFAULT 1 CHECK (event_version >= 1),
    CHECK (
        (actor_type = 'USER' AND actor_user_id IS NOT NULL)
        OR (actor_type = 'SYSTEM' AND actor_user_id IS NULL)
    ),
    -- Un camino SYSTEM no tiene sesión de usuario: la correlación solo aplica a USER.
    CHECK (actor_type = 'USER' OR session_id IS NULL),
    FOREIGN KEY (branch_id, organization_id) REFERENCES branch(id, organization_id),
    FOREIGN KEY (actor_user_id, organization_id) REFERENCES pos_user(id, organization_id),
    FOREIGN KEY (session_id, actor_user_id) REFERENCES local_session(id, user_id)
) STRICT;

CREATE INDEX idx_audit_event_org_time ON audit_event (organization_id, occurred_at DESC, id DESC);
CREATE INDEX idx_audit_event_org_branch ON audit_event (organization_id, branch_id, occurred_at DESC);
CREATE INDEX idx_audit_event_org_actor ON audit_event (organization_id, actor_user_id, occurred_at DESC);
CREATE INDEX idx_audit_event_org_entity
    ON audit_event (organization_id, entity_type, entity_id, occurred_at DESC);

-- Append-only: la aplicación no expone UPDATE/DELETE y el trigger cierra el paso
-- al acceso directo. No es inmutabilidad criptográfica frente al administrador
-- del archivo o del esquema.
CREATE TRIGGER audit_event_append_only
BEFORE UPDATE ON audit_event
BEGIN
    SELECT RAISE(ABORT, 'audit_event is append-only');
END;

CREATE TRIGGER audit_event_no_delete
BEFORE DELETE ON audit_event
BEGIN
    SELECT RAISE(ABORT, 'audit_event is append-only');
END;
