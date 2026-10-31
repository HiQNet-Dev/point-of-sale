-- Aprobada para v0.4.0 (ADR-0026). Inmutable: toda corrección posterior exige
-- una migración nueva.
-- Caja Local, propiedad de una sucursal. Depende solo de 0001 (organization/branch).
-- Restaura los campos e invariantes de Flyway V11 con nombres singulares del
-- contexto Local, como organization, branch, category y product.

CREATE TABLE cash_register (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL,
    -- Slug normalizado e inmutable, único solamente dentro de la sucursal.
    code TEXT NOT NULL CHECK (
        instr(code,char(0))=0
        AND length(code) BETWEEN 1 AND 64
        AND code NOT GLOB '*[^a-z0-9-]*'
        AND substr(code,1,1) <> '-'
        AND substr(code,-1,1) <> '-'
        AND instr(code,'--') = 0
    ),
    name TEXT NOT NULL CHECK (
        instr(name,char(0))=0 AND length(name) <= 120 AND length(trim(name)) > 0
    ),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    updated_at TEXT NOT NULL CHECK (
        updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    CONSTRAINT cash_register_branch_code_unique UNIQUE (branch_id, code),
    -- Superclave para el turno y futuras ventas scoped por org/sucursal/caja.
    CONSTRAINT cash_register_id_scope_unique UNIQUE (id, organization_id, branch_id),
    CONSTRAINT cash_register_branch_fk
        FOREIGN KEY (branch_id, organization_id) REFERENCES branch (id, organization_id)
) STRICT;

CREATE INDEX idx_cash_register_branch_name ON cash_register (branch_id, name, id);
