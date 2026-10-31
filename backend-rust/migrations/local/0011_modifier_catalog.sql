-- Aprobada para v0.4.0 (ADR-0026). Inmutable: toda corrección posterior exige
-- una migración nueva.
-- Catálogo de modificadores Local necesario para cotizar partidas y congelar sus
-- snapshots. Depende de 0008_catalog.sql; conserva ownership por
-- organización y no usa REAL para el delta monetario.

CREATE TABLE modifier_group (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    code TEXT NOT NULL CHECK (
        instr(code,char(0))=0 AND length(code) BETWEEN 1 AND 64
        AND code NOT GLOB '*[^a-z0-9-]*' AND substr(code,1,1) <> '-'
        AND substr(code,-1,1) <> '-' AND instr(code,'--')=0
    ),
    name TEXT NOT NULL CHECK (instr(name,char(0))=0 AND length(name)<=120 AND length(trim(name))>0),
    min_selections INTEGER NOT NULL DEFAULT 0 CHECK (min_selections >= 0),
    max_selections INTEGER NOT NULL DEFAULT 1 CHECK (max_selections >= 1 AND min_selections <= max_selections),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (organization_id, code),
    UNIQUE (id, organization_id)
) STRICT;

CREATE TABLE modifier_option (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    modifier_group_id TEXT NOT NULL,
    code TEXT NOT NULL CHECK (
        instr(code,char(0))=0 AND length(code) BETWEEN 1 AND 64
        AND code NOT GLOB '*[^a-z0-9-]*' AND substr(code,1,1) <> '-'
        AND substr(code,-1,1) <> '-' AND instr(code,'--')=0
    ),
    name TEXT NOT NULL CHECK (instr(name,char(0))=0 AND length(name)<=120 AND length(trim(name))>0),
    -- Decimal firmado canónico, escala cuatro y rango NUMERIC(19,4).
    price_delta TEXT NOT NULL CHECK (
        instr(price_delta,char(0))=0 AND price_delta NOT GLOB '*[^0-9.-]*'
        AND length(price_delta)-length(replace(price_delta,'.',''))=1
        AND substr(price_delta,-5,1)='.'
        AND (
            (substr(price_delta,1,1)='-' AND length(price_delta) BETWEEN 7 AND 21
             AND substr(price_delta,2,1) GLOB '[0-9]'
             AND (substr(price_delta,2,1)<>'0' OR length(price_delta)=7))
            OR
            (substr(price_delta,1,1) GLOB '[0-9]' AND length(price_delta) BETWEEN 6 AND 20
             AND (substr(price_delta,1,1)<>'0' OR length(price_delta)=6))
        )
    ),
    display_order INTEGER NOT NULL DEFAULT 0 CHECK (display_order >= 0),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (modifier_group_id, code),
    UNIQUE (id, organization_id),
    UNIQUE (id, modifier_group_id, organization_id),
    FOREIGN KEY (modifier_group_id, organization_id)
        REFERENCES modifier_group (id, organization_id)
) STRICT;

CREATE TABLE product_modifier_group (
    organization_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    modifier_group_id TEXT NOT NULL,
    display_order INTEGER NOT NULL CHECK (display_order >= 0),
    created_at TEXT NOT NULL,
    PRIMARY KEY (product_id, modifier_group_id),
    UNIQUE (product_id, display_order),
    FOREIGN KEY (product_id, organization_id) REFERENCES product (id, organization_id),
    FOREIGN KEY (modifier_group_id, organization_id) REFERENCES modifier_group (id, organization_id)
) STRICT;

CREATE INDEX idx_modifier_group_organization_name ON modifier_group (organization_id, name);
CREATE INDEX idx_modifier_option_group_order ON modifier_option (modifier_group_id, display_order, id);
CREATE INDEX idx_product_modifier_group_product_order ON product_modifier_group (product_id, display_order);
