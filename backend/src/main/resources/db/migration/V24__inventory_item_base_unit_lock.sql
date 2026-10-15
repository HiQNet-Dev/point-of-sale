-- Guarda de inmutabilidad de la unidad base del InventoryItem (v0.4.x, PR 2.3).
--
-- Política: base_unit solo puede cambiar mientras el artículo no tenga hechos
-- operativos ni dependencias históricas que fijen el significado de sus
-- cantidades. Cuando existe cualquiera de esas dependencias la unidad queda
-- congelada. El marcador unit_locked es persistente porque algunas dependencias
-- pueden borrarse (reemplazo de recetas/efectos, eliminación de partidas en
-- DRAFT) sin que el artículo deje de haber sido utilizado.
--
-- La coordinación contra carreras TOCTOU se hace en la capa de aplicación:
-- todo escritor de dependencias bloquea la fila del artículo (SELECT ... FOR
-- UPDATE) antes de leer base_unit o de insertar la dependencia, y el cambio de
-- unidad usa el mismo bloqueo. V1-V23 son inmutables.

ALTER TABLE inventory_items
    ADD COLUMN unit_locked BOOLEAN NOT NULL DEFAULT false;

-- Backfill: artículos ya referenciados por cualquier dependencia operativa o
-- histórica quedan bloqueados desde el primer arranque con V24.
UPDATE inventory_items i
SET unit_locked = true
WHERE EXISTS (
        SELECT 1 FROM branch_inventory b
        WHERE b.inventory_item_id = i.id AND b.organization_id = i.organization_id)
   OR EXISTS (
        SELECT 1 FROM inventory_movements m
        WHERE m.inventory_item_id = i.id AND m.organization_id = i.organization_id)
   OR EXISTS (
        SELECT 1 FROM product_inventory_recipe_components c
        WHERE c.inventory_item_id = i.id AND c.organization_id = i.organization_id)
   OR EXISTS (
        SELECT 1 FROM modifier_option_inventory_effects e
        WHERE e.inventory_item_id = i.id AND e.organization_id = i.organization_id)
   OR EXISTS (
        SELECT 1 FROM sale_item_inventory_components s
        WHERE s.inventory_item_id = i.id AND s.organization_id = i.organization_id)
   OR EXISTS (
        SELECT 1 FROM purchase_order_lines l
        WHERE l.inventory_item_id = i.id AND l.organization_id = i.organization_id);

COMMENT ON COLUMN inventory_items.unit_locked IS
    'Unidad base congelada: true una vez que el artículo tiene recetas, efectos de modificador, balances/movimientos de sucursal, consumo de ventas o partidas de orden de compra. El marcador es persistente aunque la dependencia se elimine.';

-- Impacto en Purchase Receipts: cuando se implementen, sus líneas y movimientos
-- de entrada también son dependencias del artículo y, por tanto, lo bloquearán
-- (o exigirán que ya esté bloqueado). No se adelanta aquí ninguna tabla ni
-- columna de recepción.
