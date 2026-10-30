-- PROPUESTA D-INV-01. No ejecutar ni promover sin aceptación formal.
-- Inventario Local candidato: artículos, saldos por sucursal y ledger append-only.
-- Depende de 0001, 0003 y 0005 candidatos.
--
-- Las cantidades se persisten como TEXT decimal canónico con escala 6, nunca
-- REAL ni INTEGER, para conservar el rango lógico NUMERIC(19,6) de Java. El
-- adaptador escribe siempre con Quantity::to_string y valida al leer.

CREATE TABLE inventory_item (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    code TEXT NOT NULL CHECK (length(code) BETWEEN 1 AND 64),
    name TEXT NOT NULL CHECK (length(name)<=120 AND length(trim(name))>0),
    description TEXT CHECK (description IS NULL OR length(description)<=1000),
    base_unit TEXT NOT NULL CHECK (base_unit IN ('UNIT','GRAM','MILLILITER')),
    fractional_unit_allowed INTEGER NOT NULL CHECK (fractional_unit_allowed IN (0,1)),
    unit_locked INTEGER NOT NULL DEFAULT 0 CHECK (unit_locked IN (0,1)),
    status TEXT NOT NULL CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    -- La unidad base solo puede congelarse, nunca volver a quedar libre.
    CHECK (unit_locked IN (0,1)),
    UNIQUE (organization_id, code),
    UNIQUE (id, organization_id)
) STRICT;

-- Saldo materializado por sucursal. La fila ausente equivale a cero; este
-- adaptador siempre crea la fila al aplicar la primera apertura.
CREATE TABLE inventory_balance (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    inventory_item_id TEXT NOT NULL,
    quantity_on_hand TEXT NOT NULL CHECK (
        instr(quantity_on_hand,char(0))=0
        AND quantity_on_hand NOT GLOB '*[^0-9.-]*'
        AND quantity_on_hand NOT GLOB '*.*.*'
        AND substr(quantity_on_hand,-7,1)='.'
        AND replace(substr(quantity_on_hand,2),'-','')=substr(quantity_on_hand,2)
        AND length(quantity_on_hand)<=21
        AND (
            quantity_on_hand GLOB '[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_on_hand GLOB '0.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_on_hand GLOB '-[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_on_hand GLOB '-0.[0-9][0-9][0-9][0-9][0-9][0-9]'
        )
    ),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_item (id, organization_id),
    FOREIGN KEY (branch_id, organization_id) REFERENCES branch (id, organization_id),
    UNIQUE (branch_id, inventory_item_id),
    UNIQUE (id, organization_id)
) STRICT;

-- Hecho inmutable del ledger. Los saldos se derivan de aquí; el trigger impide
-- reescribir la historia desde SQL directo, aunque un administrador del archivo
-- pueda desactivarlo (limitación ya registrada para la auditoría Local).
CREATE TABLE inventory_ledger (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    inventory_item_id TEXT NOT NULL,
    movement_type TEXT NOT NULL CHECK (
        movement_type IN ('OPENING','ADJUSTMENT','SALE_CONSUMPTION','PURCHASE_RECEIPT')
    ),
    quantity_delta TEXT NOT NULL CHECK (
        instr(quantity_delta,char(0))=0
        AND quantity_delta NOT GLOB '*[^0-9.-]*'
        AND quantity_delta NOT GLOB '*.*.*'
        AND substr(quantity_delta,-7,1)='.'
        AND replace(substr(quantity_delta,2),'-','')=substr(quantity_delta,2)
        AND length(quantity_delta)<=21
        AND (
            quantity_delta GLOB '[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_delta GLOB '0.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_delta GLOB '-[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_delta GLOB '-0.[0-9][0-9][0-9][0-9][0-9][0-9]'
        )
    ),
    resulting_quantity TEXT NOT NULL CHECK (
        instr(resulting_quantity,char(0))=0
        AND resulting_quantity NOT GLOB '*[^0-9.-]*'
        AND resulting_quantity NOT GLOB '*.*.*'
        AND substr(resulting_quantity,-7,1)='.'
        AND replace(substr(resulting_quantity,2),'-','')=substr(resulting_quantity,2)
        AND length(resulting_quantity)<=21
        AND (
            resulting_quantity GLOB '[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR resulting_quantity GLOB '0.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR resulting_quantity GLOB '-[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR resulting_quantity GLOB '-0.[0-9][0-9][0-9][0-9][0-9][0-9]'
        )
    ),
    reason TEXT CHECK (
        reason IS NULL OR (length(reason) BETWEEN 1 AND 255 AND length(trim(reason))>0)
    ),
    actor_user_id TEXT NOT NULL,
    occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
    sale_id TEXT,
    purchase_receipt_line_id TEXT,
    FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_item (id, organization_id),
    FOREIGN KEY (branch_id, organization_id) REFERENCES branch (id, organization_id),
    FOREIGN KEY (actor_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    CHECK (
        (movement_type='OPENING'
            AND substr(quantity_delta,1,1)<>'-'
            AND reason IS NULL AND sale_id IS NULL AND purchase_receipt_line_id IS NULL)
        OR (movement_type='ADJUSTMENT'
            AND reason IS NOT NULL AND sale_id IS NULL AND purchase_receipt_line_id IS NULL
            AND quantity_delta NOT IN ('0.000000','-0.000000'))
        OR (movement_type='SALE_CONSUMPTION'
            AND substr(quantity_delta,1,1)='-'
            AND reason IS NULL AND sale_id IS NOT NULL AND purchase_receipt_line_id IS NULL)
        OR (movement_type='PURCHASE_RECEIPT'
            AND substr(quantity_delta,1,1)<>'-'
            AND reason IS NULL AND sale_id IS NULL AND purchase_receipt_line_id IS NOT NULL
            AND quantity_delta NOT IN ('0.000000'))
    ),
    UNIQUE (id, organization_id)
) STRICT;

CREATE TRIGGER inventory_ledger_append_only
BEFORE UPDATE ON inventory_ledger
BEGIN
    SELECT RAISE(ABORT, 'inventory_ledger is append-only');
END;

CREATE TRIGGER inventory_ledger_no_delete
BEFORE DELETE ON inventory_ledger
BEGIN
    SELECT RAISE(ABORT, 'inventory_ledger is append-only');
END;

CREATE INDEX idx_inventory_ledger_item_occurred
    ON inventory_ledger (organization_id, branch_id, inventory_item_id, occurred_at, id);
CREATE INDEX idx_inventory_ledger_sale
    ON inventory_ledger (sale_id, inventory_item_id);
