-- PROPUESTA 2B. No ejecutar: requiere aprobación física completa.
--
-- Compras: proveedores, órdenes, partidas y recepciones.
--
-- Depende de 0001 (organization, branch), 0003 (pos_user), 0006 (audit_event),
-- 0013 (inventory_item, inventory_balance, inventory_ledger) y 0014 (recetas).
--
-- Decisiones aplicadas:
--   * CC21-03 aprobada el 2026-10-30: `unit_cost` es NULL-able. NULL significa
--     costo pactado desconocido; `0.0000` es un costo conocido igual a cero.
--   * El total de la orden no se almacena: se deriva y es NULL si alguna partida
--     desconoce su costo.
--   * Las recepciones no guardan valoración económica (ADR-0021 sigue pendiente
--     en costo efectivo, valoración histórica e import).
--
-- Todas las tablas son aditivas; ninguna migración histórica se modifica.

-- ---------------------------------------------------------------------------
-- Proveedores: dato maestro de la organización, nunca se elimina.
-- ---------------------------------------------------------------------------
CREATE TABLE supplier (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    -- Slug inmutable, único por organización.
    code TEXT NOT NULL CHECK (
        instr(code,char(0))=0
        AND length(code) BETWEEN 1 AND 64
        AND code NOT GLOB '*[^a-z0-9-]*'
        AND substr(code,1,1) <> '-'
        AND substr(code,-1,1) <> '-'
    ),
    name TEXT NOT NULL CHECK (
        instr(name,char(0))=0 AND length(name) <= 120 AND length(trim(name)) > 0
    ),
    legal_name TEXT CHECK (
        legal_name IS NULL
        OR (instr(legal_name,char(0))=0 AND length(legal_name) <= 160 AND length(trim(legal_name)) > 0)
    ),
    tax_id TEXT CHECK (
        tax_id IS NULL
        OR (instr(tax_id,char(0))=0 AND length(tax_id) <= 64 AND length(trim(tax_id)) > 0)
    ),
    contact_name TEXT CHECK (
        contact_name IS NULL
        OR (instr(contact_name,char(0))=0 AND length(contact_name) <= 120 AND length(trim(contact_name)) > 0)
    ),
    -- El email se persiste ya normalizado a minúsculas por la aplicación.
    email TEXT CHECK (
        email IS NULL
        OR (
            instr(email,char(0))=0
            AND length(email) <= 254
            AND email NOT GLOB '*[^ -~]*'
            AND email GLOB '*@*.*'
        )
    ),
    phone TEXT CHECK (
        phone IS NULL
        OR (instr(phone,char(0))=0 AND length(phone) <= 40 AND length(trim(phone)) > 0)
    ),
    address TEXT CHECK (
        address IS NULL
        OR (instr(address,char(0))=0 AND length(address) <= 500 AND length(trim(address)) > 0)
    ),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CONSTRAINT supplier_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT supplier_id_organization_unique UNIQUE (organization_id, id)
) STRICT;

CREATE INDEX idx_supplier_organization_name ON supplier (organization_id, name);

-- ---------------------------------------------------------------------------
-- Órdenes de compra: pertenecen a una sucursal de la organización.
-- ---------------------------------------------------------------------------
CREATE TABLE purchase_order (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL,
    supplier_id TEXT NOT NULL,
    code TEXT NOT NULL CHECK (
        instr(code,char(0))=0
        AND length(code) BETWEEN 1 AND 64
        AND code NOT GLOB '*[^a-z0-9-]*'
        AND substr(code,1,1) <> '-'
        AND substr(code,-1,1) <> '-'
    ),
    status TEXT NOT NULL DEFAULT 'DRAFT'
        CHECK (status IN ('DRAFT','ORDERED','PARTIALLY_RECEIVED','RECEIVED','CANCELLED')),
    currency TEXT NOT NULL CHECK (
        instr(currency,char(0))=0
        AND length(currency)=3
        AND currency NOT GLOB '*[^A-Z]*'
    ),
    supplier_code_snapshot TEXT NOT NULL CHECK (
        instr(supplier_code_snapshot,char(0))=0
        AND length(supplier_code_snapshot) BETWEEN 1 AND 64
    ),
    supplier_name_snapshot TEXT NOT NULL CHECK (
        instr(supplier_name_snapshot,char(0))=0
        AND length(supplier_name_snapshot) <= 120
        AND length(trim(supplier_name_snapshot)) > 0
    ),
    created_by_user_id TEXT NOT NULL,
    issued_by_user_id TEXT,
    cancelled_by_user_id TEXT,
    issued_at INTEGER CHECK (issued_at IS NULL OR issued_at >= 0),
    cancelled_at INTEGER CHECK (cancelled_at IS NULL OR cancelled_at >= 0),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CONSTRAINT purchase_order_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT purchase_order_id_organization_unique UNIQUE (organization_id, id),
    -- Superclave para que partidas y recepciones no crucen organización ni sucursal.
    CONSTRAINT purchase_order_scope_unique UNIQUE (organization_id, branch_id, id),
    CONSTRAINT purchase_order_branch_fk
        FOREIGN KEY (branch_id, organization_id) REFERENCES branch (id, organization_id),
    CONSTRAINT purchase_order_supplier_fk
        FOREIGN KEY (supplier_id, organization_id) REFERENCES supplier (id, organization_id),
    CONSTRAINT purchase_order_creator_fk
        FOREIGN KEY (created_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    -- MATCH SIMPLE: si el actor es NULL la FK no se evalúa; si existe, debe ser
    -- de la misma organización.
    CONSTRAINT purchase_order_issuer_fk
        FOREIGN KEY (issued_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    CONSTRAINT purchase_order_canceller_fk
        FOREIGN KEY (cancelled_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    -- Coherencia de estados finales y de emisión.
    CHECK (
        (status = 'DRAFT' AND issued_at IS NULL AND issued_by_user_id IS NULL)
        OR (status IN ('ORDERED','PARTIALLY_RECEIVED','RECEIVED')
            AND issued_at IS NOT NULL AND issued_by_user_id IS NOT NULL)
        OR status = 'CANCELLED'
    )
) STRICT;

CREATE INDEX idx_purchase_order_branch_status
    ON purchase_order (organization_id, branch_id, status, created_at);

-- ---------------------------------------------------------------------------
-- Partidas: snapshots congelados y costo pactado opcional (CC21-03).
-- ---------------------------------------------------------------------------
CREATE TABLE purchase_order_line (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    purchase_order_id TEXT NOT NULL,
    inventory_item_id TEXT NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    item_code_snapshot TEXT NOT NULL CHECK (
        instr(item_code_snapshot,char(0))=0
        AND length(item_code_snapshot) BETWEEN 1 AND 64
        AND length(trim(item_code_snapshot)) > 0
    ),
    item_name_snapshot TEXT NOT NULL CHECK (
        instr(item_name_snapshot,char(0))=0
        AND length(item_name_snapshot) <= 120
        AND length(trim(item_name_snapshot)) > 0
    ),
    unit_snapshot TEXT NOT NULL CHECK (
        unit_snapshot IN ('UNIT','GRAM','MILLILITER')
    ),
    -- Cantidad pedida: escala 6, estrictamente positiva.
    quantity TEXT NOT NULL CHECK (
        instr(quantity,char(0))=0
        AND quantity NOT GLOB '*[^0-9.]*'
        AND length(quantity) BETWEEN 8 AND 20
        AND length(quantity) - length(replace(quantity,'.','')) = 1
        AND substr(quantity,-7,1) = '.'
        AND quantity NOT IN ('0.000000')
        AND (substr(quantity,1,1) <> '0' OR length(quantity) = 8)
    ),
    -- CC21-03: NULL = costo pactado desconocido. `0.0000` es un cero conocido.
    unit_cost TEXT CHECK (
        unit_cost IS NULL
        OR (
            instr(unit_cost,char(0))=0
            AND unit_cost NOT GLOB '*[^0-9.]*'
            AND length(unit_cost) BETWEEN 6 AND 20
            AND length(unit_cost) - length(replace(unit_cost,'.','')) = 1
            AND substr(unit_cost,-5,1) = '.'
            AND (substr(unit_cost,1,1) <> '0' OR length(unit_cost) = 6)
        )
    ),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CONSTRAINT purchase_order_line_order_fk
        FOREIGN KEY (purchase_order_id, organization_id)
        REFERENCES purchase_order (id, organization_id) ON DELETE CASCADE,
    CONSTRAINT purchase_order_line_item_fk
        FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_item (id, organization_id),
    CONSTRAINT purchase_order_line_number_unique UNIQUE (purchase_order_id, line_number),
    CONSTRAINT purchase_order_line_id_organization_unique UNIQUE (organization_id, id)
) STRICT;

-- ---------------------------------------------------------------------------
-- Recepciones: hechos inmutables; el ledger de inventario las referencia.
-- ---------------------------------------------------------------------------
CREATE TABLE purchase_receipt (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    purchase_order_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL CHECK (
        instr(idempotency_key,char(0))=0
        AND length(idempotency_key) BETWEEN 1 AND 120
        AND length(trim(idempotency_key)) > 0
    ),
    actor_user_id TEXT NOT NULL,
    received_at INTEGER NOT NULL CHECK (received_at >= 0),
    created_at TEXT NOT NULL,
    CONSTRAINT purchase_receipt_idempotency_unique UNIQUE (organization_id, idempotency_key),
    CONSTRAINT purchase_receipt_id_organization_unique UNIQUE (organization_id, id),
    CONSTRAINT purchase_receipt_order_fk
        FOREIGN KEY (purchase_order_id, branch_id, organization_id)
        REFERENCES purchase_order (id, branch_id, organization_id),
    CONSTRAINT purchase_receipt_actor_fk
        FOREIGN KEY (actor_user_id, organization_id) REFERENCES pos_user (id, organization_id)
) STRICT;

CREATE INDEX idx_purchase_receipt_order
    ON purchase_receipt (organization_id, purchase_order_id, received_at);

CREATE TABLE purchase_receipt_line (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    purchase_receipt_id TEXT NOT NULL,
    purchase_order_line_id TEXT NOT NULL,
    inventory_item_id TEXT NOT NULL,
    quantity TEXT NOT NULL CHECK (
        instr(quantity,char(0))=0
        AND quantity NOT GLOB '*[^0-9.]*'
        AND length(quantity) BETWEEN 8 AND 20
        AND length(quantity) - length(replace(quantity,'.','')) = 1
        AND substr(quantity,-7,1) = '.'
        AND quantity NOT IN ('0.000000')
        AND (substr(quantity,1,1) <> '0' OR length(quantity) = 8)
    ),
    created_at TEXT NOT NULL,
    CONSTRAINT purchase_receipt_line_receipt_fk
        FOREIGN KEY (purchase_receipt_id, organization_id)
        REFERENCES purchase_receipt (id, organization_id) ON DELETE CASCADE,
    CONSTRAINT purchase_receipt_line_order_line_fk
        FOREIGN KEY (purchase_order_line_id, organization_id)
        REFERENCES purchase_order_line (id, organization_id),
    CONSTRAINT purchase_receipt_line_item_fk
        FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_item (id, organization_id),
    CONSTRAINT purchase_receipt_line_unique UNIQUE (purchase_receipt_id, purchase_order_line_id)
) STRICT;

CREATE INDEX idx_purchase_receipt_line_order_line
    ON purchase_receipt_line (organization_id, purchase_order_line_id);
