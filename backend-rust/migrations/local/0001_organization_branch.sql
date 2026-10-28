-- Subconjunto de F1–10 §16.1–2; NO es el esquema Local completo ni aprueba IMP-01.
-- No fija moneda, zona horaria ni locale por defecto: deben ser decisiones explícitas.
CREATE TABLE organization (
    -- length/GLOB sobre TEXT se detienen en NUL: rechazarlo antes de comprobar formato.
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    currency TEXT NOT NULL CHECK (instr(currency,char(0))=0 AND length(currency)=3 AND currency NOT GLOB '*[^A-Z]*'),
    timezone TEXT NOT NULL,
    locale TEXT NOT NULL,
    stock_negative_policy TEXT NOT NULL CHECK (stock_negative_policy IN ('STRICT','ALLOW_NEGATIVE')),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE branch (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    timezone_override TEXT,
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (organization_id,code),
    -- Clave de referencia para futuros hechos scoped; no limita sucursales históricas.
    UNIQUE (organization_id,id)
) STRICT;
