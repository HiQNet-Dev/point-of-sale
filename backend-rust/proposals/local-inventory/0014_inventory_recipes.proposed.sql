-- PROPUESTA D-INV-02. No ejecutar ni promover sin aceptación formal.
-- Recetas de producto y efectos de modificadores sobre inventario.
-- Depende de 0008 (product), 0011 (modifier_option) y 0013 (inventory_item).
--
-- Una receta ACTIVE exige al menos un componente: el dominio
-- (pos-core::recipe::plan_recipe_replacement) rechaza la combinación vacía y
-- este CHECK evita que una escritura directa la introduzca.
-- Los snapshots históricos de consumo viven en 0015, dentro de la línea de venta.

CREATE TABLE product_recipe (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (product_id, organization_id) REFERENCES product (id, organization_id),
    UNIQUE (organization_id, product_id),
    UNIQUE (id, organization_id)
) STRICT;

CREATE TABLE recipe_component (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    inventory_item_id TEXT NOT NULL,
    quantity_per_product TEXT NOT NULL CHECK (
        instr(quantity_per_product,char(0))=0
        AND quantity_per_product NOT GLOB '*[^0-9.-]*'
        AND quantity_per_product NOT GLOB '*.*.*'
        AND quantity_per_product NOT GLOB '*-*'
        AND substr(quantity_per_product,-7,1)='.'
        AND length(quantity_per_product)<=20
        AND quantity_per_product NOT IN ('0.000000','-0.000000')
        AND (
            quantity_per_product GLOB '[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_per_product GLOB '0.[0-9][0-9][0-9][0-9][0-9][0-9]'
        )
    ),
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL,
    FOREIGN KEY (recipe_id, organization_id) REFERENCES product_recipe (id, organization_id),
    FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_item (id, organization_id),
    UNIQUE (recipe_id, inventory_item_id),
    UNIQUE (recipe_id, position)
) STRICT;

CREATE TABLE modifier_inventory_effect (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL,
    modifier_option_id TEXT NOT NULL,
    inventory_item_id TEXT NOT NULL,
    quantity_delta TEXT NOT NULL CHECK (
        instr(quantity_delta,char(0))=0
        AND quantity_delta NOT GLOB '*[^0-9.-]*'
        AND quantity_delta NOT GLOB '*.*.*'
        AND substr(quantity_delta,-7,1)='.'
        AND replace(substr(quantity_delta,2),'-','')=substr(quantity_delta,2)
        AND length(quantity_delta)<=21
        AND quantity_delta NOT IN ('0.000000','-0.000000')
        AND (
            quantity_delta GLOB '[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_delta GLOB '0.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_delta GLOB '-[1-9]*.[0-9][0-9][0-9][0-9][0-9][0-9]'
            OR quantity_delta GLOB '-0.[0-9][0-9][0-9][0-9][0-9][0-9]'
        )
    ),
    created_at TEXT NOT NULL,
    FOREIGN KEY (modifier_option_id, organization_id)
        REFERENCES modifier_option (id, organization_id),
    FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_item (id, organization_id),
    UNIQUE (modifier_option_id, inventory_item_id)
) STRICT;

CREATE INDEX idx_recipe_component_recipe_position
    ON recipe_component (recipe_id, position);
CREATE INDEX idx_modifier_inventory_effect_option
    ON modifier_inventory_effect (modifier_option_id);
