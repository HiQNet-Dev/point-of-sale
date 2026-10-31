-- PROPUESTA. No ejecutar. No está cargada por `pos-local` ni por SQLx.
-- Catálogo Local: `category` y `product`, propiedad de la ORGANIZACIÓN.
--
-- Depende únicamente de `0001_organization_branch.sql`. No depende de
-- D-AUTH/D-INST: no referencia `pos_user`, `local_session` ni `audit_event`.
--
-- Paridad con Flyway V9 (`categories`/`products`): mismos campos, mismas
-- unicidades y misma barrera de tenant. Diferencias deliberadas del contexto
-- Local, todas pendientes de aprobación:
--
--   1. Nombres en singular (`category`/`product`), como `organization`/`branch`.
--   2. `unit_price` TEXT canónico con escala 4 en lugar de `NUMERIC(19,4)`:
--      el rango lógico de `NUMERIC(19,4)` (magnitud < 10^19) excede `i64`, y
--      SQLite no tiene decimal exacto. El texto canónico `0.0000` conserva
--      exactitud y rango completo sin usar REAL.
--   3. Sin columnas generadas ni triggers de catálogo: la retirada es
--      `status = 'INACTIVE'` y nunca propaga a otra fila.
--
-- No hay borrado físico: la API de producto retira con INACTIVE.

CREATE TABLE category (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    -- Slug normalizado por la aplicación; inmutable y único por organización.
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
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CONSTRAINT category_organization_code_unique UNIQUE (organization_id, code),
    -- Superclave que habilita la FK same-organization desde `product`.
    CONSTRAINT category_id_organization_unique UNIQUE (organization_id, id)
) STRICT;

CREATE TABLE product (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    category_id TEXT,
    -- Identificador de negocio: mayúsculas canónicas, inmutable, único por
    -- organización. No sustituye al UUID.
    sku TEXT NOT NULL CHECK (
        instr(sku,char(0))=0
        AND length(sku) BETWEEN 1 AND 64
        AND sku NOT GLOB '*[^A-Z0-9._-]*'
    ),
    -- Opcional; se conserva como texto para preservar ceros iniciales.
    barcode TEXT CHECK (
        barcode IS NULL
        OR (
            instr(barcode,char(0))=0
            AND length(barcode) BETWEEN 1 AND 64
            AND length(trim(barcode)) > 0
        )
    ),
    name TEXT NOT NULL CHECK (
        instr(name,char(0))=0 AND length(name) <= 120 AND length(trim(name)) > 0
    ),
    description TEXT CHECK (
        description IS NULL
        OR (
            instr(description,char(0))=0
            AND length(description) <= 1000
            AND length(trim(description)) > 0
        )
    ),
    -- Precio base de la organización: decimal canónico no negativo, escala 4.
    -- Forma admitida: `0.0000`, `123.4500`, `999999999999999.9999`.
    unit_price TEXT NOT NULL CHECK (
        instr(unit_price,char(0))=0
        AND unit_price NOT GLOB '*[^0-9.]*'
        AND length(unit_price) BETWEEN 6 AND 20
        AND length(unit_price) - length(replace(unit_price,'.','')) = 1
        AND substr(unit_price,-5,1) = '.'
        AND (substr(unit_price,1,1) <> '0' OR length(unit_price) = 6)
    ),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CONSTRAINT product_organization_sku_unique UNIQUE (organization_id, sku),
    CONSTRAINT product_id_organization_unique UNIQUE (organization_id, id),
    -- Tenant a nivel de base de datos: si `category_id` es NULL la FK no se
    -- evalúa (MATCH SIMPLE); si existe, la categoría debe ser de la MISMA
    -- organización. La validación Java no es la única barrera.
    CONSTRAINT product_category_fk
        FOREIGN KEY (category_id, organization_id) REFERENCES category (id, organization_id)
) STRICT;

-- Único por organización solo cuando existe (índice parcial único).
CREATE UNIQUE INDEX product_organization_barcode_unique
    ON product (organization_id, barcode)
    WHERE barcode IS NOT NULL;

CREATE INDEX idx_category_organization_name ON category (organization_id, name);
CREATE INDEX idx_product_organization_name ON product (organization_id, name);
CREATE INDEX idx_product_organization_category ON product (organization_id, category_id);
