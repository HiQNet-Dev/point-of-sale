-- PROPUESTA D-AUTH-06. No ejecutar sin aceptación formal.
-- Depende de 0002 (pos_user) y 0001 (organization).

CREATE TABLE permission (
    code TEXT PRIMARY KEY NOT NULL CHECK (
        length(code) BETWEEN 3 AND 64 AND code NOT GLOB '*[^a-z._]*'
    ),
    description TEXT NOT NULL CHECK (length(trim(description)) > 0)
) STRICT;

CREATE TABLE role (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    code TEXT NOT NULL CHECK (
        length(code) BETWEEN 1 AND 64 AND code NOT GLOB '*[^a-z0-9-]*'
    ),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0 AND length(name) <= 120),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (organization_id, code),
    UNIQUE (id, organization_id)
) STRICT;

CREATE TABLE role_permission (
    role_id TEXT NOT NULL REFERENCES role(id),
    permission_code TEXT NOT NULL REFERENCES permission(code),
    PRIMARY KEY (role_id, permission_code)
) STRICT;

CREATE TABLE user_organization_role (
    organization_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (organization_id, user_id, role_id),
    FOREIGN KEY (user_id, organization_id) REFERENCES pos_user(id, organization_id),
    FOREIGN KEY (role_id, organization_id) REFERENCES role(id, organization_id)
) STRICT;

CREATE INDEX idx_role_organization ON role (organization_id, status);
CREATE INDEX idx_user_organization_role_user ON user_organization_role (organization_id, user_id);

-- El catálogo de permisos (59 códigos de V5-V25) se siembra desde la fuente
-- aprobada; no se enumeran códigos aquí para no inventar concesiones.
