-- PROPUESTA D-AUTH-06 (memberships). No ejecutar sin aceptación formal.
-- Depende de 0001 (branch), 0002 (pos_user) y 0003 (role).

CREATE TABLE branch_membership (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (organization_id, branch_id, user_id),
    UNIQUE (id, organization_id),
    FOREIGN KEY (branch_id, organization_id) REFERENCES branch(id, organization_id),
    FOREIGN KEY (user_id, organization_id) REFERENCES pos_user(id, organization_id)
) STRICT;

CREATE TABLE branch_membership_role (
    organization_id TEXT NOT NULL,
    membership_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (organization_id, membership_id, role_id),
    FOREIGN KEY (membership_id, organization_id)
        REFERENCES branch_membership(id, organization_id),
    FOREIGN KEY (role_id, organization_id) REFERENCES role(id, organization_id)
) STRICT;

CREATE INDEX idx_branch_membership_branch ON branch_membership (organization_id, branch_id);
CREATE INDEX idx_branch_membership_user ON branch_membership (organization_id, user_id);
