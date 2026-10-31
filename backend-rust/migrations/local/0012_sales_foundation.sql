-- Aprobada para v0.4.0 (ADR-0026). Inmutable: toda corrección posterior exige
-- una migración nueva.
-- Ventas Local: OPEN, CANCELLED y COMPLETED ya están representados
-- para que pagos/checkout sean aditivos. Este incremento solo autoriza OPEN y
-- CANCELLED: no existe una ruta que pueda completar sin pagos e inventario.
--
-- Depende de 0001, 0003, 0008, 0009, 0010 y 0011 candidatos.

CREATE TABLE sale (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL,
    cash_register_id TEXT NOT NULL,
    cash_shift_id TEXT NOT NULL,
    created_by_user_id TEXT NOT NULL,
    cancelled_by_user_id TEXT,
    completed_by_user_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('OPEN','CANCELLED','COMPLETED')),
    total_amount TEXT NOT NULL CHECK (
        instr(total_amount,char(0))=0 AND total_amount NOT GLOB '*[^0-9.]*'
        AND substr(total_amount,1,1) GLOB '[0-9]'
        AND length(total_amount) BETWEEN 6 AND 20
        AND length(total_amount)-length(replace(total_amount,'.',''))=1
        AND substr(total_amount,-5,1)='.'
        AND (substr(total_amount,1,1)<>'0' OR length(total_amount)=6)
    ),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    cancelled_at TEXT,
    completed_at TEXT,
    FOREIGN KEY (cash_shift_id, organization_id, branch_id, cash_register_id)
        REFERENCES cash_shift (id, organization_id, branch_id, cash_register_id),
    FOREIGN KEY (created_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    FOREIGN KEY (cancelled_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    FOREIGN KEY (completed_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    CHECK (
        (status='OPEN' AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL AND completed_at IS NULL AND completed_by_user_id IS NULL)
        OR (status='CANCELLED' AND cancelled_at IS NOT NULL AND cancelled_by_user_id IS NOT NULL AND completed_at IS NULL AND completed_by_user_id IS NULL)
        OR (status='COMPLETED' AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL AND completed_at IS NOT NULL AND completed_by_user_id IS NOT NULL)
    ),
    UNIQUE (id, organization_id),
    UNIQUE (id, organization_id, branch_id, cash_register_id, cash_shift_id)
) STRICT;

CREATE TABLE sale_item (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    sale_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    product_sku_snapshot TEXT NOT NULL CHECK (length(product_sku_snapshot) BETWEEN 1 AND 64),
    product_name_snapshot TEXT NOT NULL CHECK (length(product_name_snapshot)<=120 AND length(trim(product_name_snapshot))>0),
    base_unit_price TEXT NOT NULL CHECK (
        base_unit_price NOT GLOB '*[^0-9.]*' AND substr(base_unit_price,1,1) GLOB '[0-9]'
        AND length(base_unit_price) BETWEEN 6 AND 20 AND length(base_unit_price)-length(replace(base_unit_price,'.',''))=1
        AND substr(base_unit_price,-5,1)='.' AND (substr(base_unit_price,1,1)<>'0' OR length(base_unit_price)=6)
    ),
    modifier_unit_delta TEXT NOT NULL CHECK (
        modifier_unit_delta NOT GLOB '*[^0-9.-]*' AND length(modifier_unit_delta)-length(replace(modifier_unit_delta,'.',''))=1
        AND substr(modifier_unit_delta,-5,1)='.' AND (
            (substr(modifier_unit_delta,1,1)='-' AND length(modifier_unit_delta) BETWEEN 7 AND 21 AND substr(modifier_unit_delta,2,1) GLOB '[0-9]' AND (substr(modifier_unit_delta,2,1)<>'0' OR length(modifier_unit_delta)=7))
            OR (substr(modifier_unit_delta,1,1) GLOB '[0-9]' AND length(modifier_unit_delta) BETWEEN 6 AND 20 AND (substr(modifier_unit_delta,1,1)<>'0' OR length(modifier_unit_delta)=6))
        )
    ),
    effective_unit_price TEXT NOT NULL CHECK (
        effective_unit_price NOT GLOB '*[^0-9.]*' AND substr(effective_unit_price,1,1) GLOB '[0-9]'
        AND length(effective_unit_price) BETWEEN 6 AND 20 AND length(effective_unit_price)-length(replace(effective_unit_price,'.',''))=1
        AND substr(effective_unit_price,-5,1)='.' AND (substr(effective_unit_price,1,1)<>'0' OR length(effective_unit_price)=6)
    ),
    quantity INTEGER NOT NULL CHECK (quantity BETWEEN 1 AND 1000000),
    line_total TEXT NOT NULL CHECK (
        line_total NOT GLOB '*[^0-9.]*' AND substr(line_total,1,1) GLOB '[0-9]'
        AND length(line_total) BETWEEN 6 AND 20 AND length(line_total)-length(replace(line_total,'.',''))=1
        AND substr(line_total,-5,1)='.' AND (substr(line_total,1,1)<>'0' OR length(line_total)=6)
    ),
    status TEXT NOT NULL CHECK (status IN ('ACTIVE','REMOVED')),
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    removed_at TEXT,
    removed_by_user_id TEXT,
    FOREIGN KEY (sale_id, organization_id) REFERENCES sale (id, organization_id),
    FOREIGN KEY (product_id, organization_id) REFERENCES product (id, organization_id),
    FOREIGN KEY (removed_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    CHECK (
        (status='ACTIVE' AND removed_at IS NULL AND removed_by_user_id IS NULL)
        OR (status='REMOVED' AND removed_at IS NOT NULL AND removed_by_user_id IS NOT NULL)
    ),
    UNIQUE (sale_id, position),
    UNIQUE (id, organization_id)
) STRICT;

CREATE TABLE sale_item_modifier (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    sale_item_id TEXT NOT NULL,
    modifier_group_id TEXT NOT NULL,
    modifier_option_id TEXT NOT NULL,
    group_code_snapshot TEXT NOT NULL CHECK (length(group_code_snapshot) BETWEEN 1 AND 64),
    group_name_snapshot TEXT NOT NULL CHECK (length(group_name_snapshot)<=120 AND length(trim(group_name_snapshot))>0),
    option_code_snapshot TEXT NOT NULL CHECK (length(option_code_snapshot) BETWEEN 1 AND 64),
    option_name_snapshot TEXT NOT NULL CHECK (length(option_name_snapshot)<=120 AND length(trim(option_name_snapshot))>0),
    price_delta_snapshot TEXT NOT NULL CHECK (
        price_delta_snapshot NOT GLOB '*[^0-9.-]*' AND length(price_delta_snapshot)-length(replace(price_delta_snapshot,'.',''))=1
        AND substr(price_delta_snapshot,-5,1)='.' AND (
            (substr(price_delta_snapshot,1,1)='-' AND length(price_delta_snapshot) BETWEEN 7 AND 21 AND substr(price_delta_snapshot,2,1) GLOB '[0-9]' AND (substr(price_delta_snapshot,2,1)<>'0' OR length(price_delta_snapshot)=7))
            OR (substr(price_delta_snapshot,1,1) GLOB '[0-9]' AND length(price_delta_snapshot) BETWEEN 6 AND 20 AND (substr(price_delta_snapshot,1,1)<>'0' OR length(price_delta_snapshot)=6))
        )
    ),
    display_order INTEGER NOT NULL CHECK (display_order >= 0),
    created_at TEXT NOT NULL,
    FOREIGN KEY (sale_item_id, organization_id) REFERENCES sale_item (id, organization_id),
    FOREIGN KEY (modifier_group_id, organization_id) REFERENCES modifier_group (id, organization_id),
    FOREIGN KEY (modifier_option_id, modifier_group_id, organization_id)
        REFERENCES modifier_option (id, modifier_group_id, organization_id),
    UNIQUE (sale_item_id, modifier_option_id)
) STRICT;

CREATE INDEX idx_sale_shift_status_created ON sale (cash_shift_id, status, created_at DESC, id DESC);
CREATE INDEX idx_sale_item_sale_status_position ON sale_item (sale_id, status, position);
CREATE INDEX idx_sale_item_modifier_item_order ON sale_item_modifier (sale_item_id, display_order, id);
