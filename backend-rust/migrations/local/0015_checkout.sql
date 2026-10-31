-- Aprobada para v0.4.0 (ADR-0026). Inmutable: toda corrección posterior exige
-- una migración nueva.
-- Checkout Local: pagos inmutables y snapshots de consumo por línea.
-- Depende de 0012 (sale/sale_item), 0003 (pos_user) y 0013 (inventory_item).
--
-- Un pago pertenece a una venta y hereda su scope completo mediante la
-- superclave de 0012, de modo que SQLite rechaza cobros de otra caja, sucursal
-- u organización. Los importes son TEXT decimal canónico escala 4.

CREATE TABLE payment (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    cash_register_id TEXT NOT NULL,
    cash_shift_id TEXT NOT NULL,
    sale_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 10),
    method TEXT NOT NULL CHECK (method IN ('CASH','CARD')),
    amount TEXT NOT NULL CHECK (
        instr(amount,char(0))=0 AND amount NOT GLOB '*[^0-9.]*'
        AND substr(amount,1,1) GLOB '[0-9]'
        AND length(amount) BETWEEN 6 AND 20
        AND length(amount)-length(replace(amount,'.',''))=1
        AND substr(amount,-5,1)='.'
        AND (substr(amount,1,1)<>'0' OR length(amount)=6)
        AND amount NOT IN ('0.0000')
    ),
    tendered_amount TEXT CHECK (
        tendered_amount IS NULL OR (
            instr(tendered_amount,char(0))=0 AND tendered_amount NOT GLOB '*[^0-9.]*'
            AND substr(tendered_amount,1,1) GLOB '[0-9]'
            AND length(tendered_amount) BETWEEN 6 AND 20
            AND length(tendered_amount)-length(replace(tendered_amount,'.',''))=1
            AND substr(tendered_amount,-5,1)='.'
            AND (substr(tendered_amount,1,1)<>'0' OR length(tendered_amount)=6)
            AND tendered_amount NOT IN ('0.0000')
        )
    ),
    change_amount TEXT NOT NULL CHECK (
        instr(change_amount,char(0))=0 AND change_amount NOT GLOB '*[^0-9.]*'
        AND substr(change_amount,1,1) GLOB '[0-9]'
        AND length(change_amount) BETWEEN 6 AND 20
        AND length(change_amount)-length(replace(change_amount,'.',''))=1
        AND substr(change_amount,-5,1)='.'
        AND (substr(change_amount,1,1)<>'0' OR length(change_amount)=6)
    ),
    created_by_user_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (sale_id, organization_id, branch_id, cash_register_id, cash_shift_id)
        REFERENCES sale (id, organization_id, branch_id, cash_register_id, cash_shift_id),
    FOREIGN KEY (created_by_user_id, organization_id)
        REFERENCES pos_user (id, organization_id),
    CHECK (
        (method='CASH' AND tendered_amount IS NOT NULL)
        OR (method='CARD' AND tendered_amount IS NULL)
    ),
    UNIQUE (sale_id, position),
    UNIQUE (id, organization_id)
) STRICT;

-- Snapshot de consumo congelado por línea (equivalente a V19): la historia no
-- depende de la receta vigente ni de los efectos actuales del modificador.
CREATE TABLE sale_item_inventory_component (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    sale_item_id TEXT NOT NULL,
    inventory_item_id TEXT NOT NULL,
    base_unit TEXT NOT NULL CHECK (base_unit IN ('UNIT','GRAM','MILLILITER')),
    quantity_per_unit TEXT NOT NULL CHECK (
        instr(quantity_per_unit,char(0))=0
        AND quantity_per_unit NOT GLOB '*[^0-9.-]*'
        AND quantity_per_unit NOT GLOB '*.*.*'
        AND quantity_per_unit NOT GLOB '*-*'
        AND substr(quantity_per_unit,-7,1)='.'
        AND length(quantity_per_unit)<=20
        AND quantity_per_unit NOT IN ('0.000000','-0.000000')
        AND (
            quantity_per_unit GLOB '[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_per_unit GLOB '0.[0-9][0-9][0-9][0-9][0-9][0-9]'
        )
    ),
    quantity_total TEXT NOT NULL CHECK (
        instr(quantity_total,char(0))=0
        AND quantity_total NOT GLOB '*[^0-9.-]*'
        AND quantity_total NOT GLOB '*.*.*'
        AND quantity_total NOT GLOB '*-*'
        AND substr(quantity_total,-7,1)='.'
        AND length(quantity_total)<=20
        AND quantity_total NOT IN ('0.000000','-0.000000')
        AND (
            quantity_total GLOB '[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_total GLOB '0.[0-9][0-9][0-9][0-9][0-9][0-9]'
        )
    ),
    created_at TEXT NOT NULL,
    FOREIGN KEY (sale_item_id, organization_id) REFERENCES sale_item (id, organization_id),
    FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_item (id, organization_id),
    UNIQUE (sale_item_id, inventory_item_id)
) STRICT;

CREATE INDEX idx_payment_sale_position ON payment (sale_id, position);
CREATE INDEX idx_sale_item_inventory_component_item
    ON sale_item_inventory_component (inventory_item_id);
