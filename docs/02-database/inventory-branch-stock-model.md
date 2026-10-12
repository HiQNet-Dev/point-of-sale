# Branch inventory stock — V17

**Estado:** candidate / PR abierto. V17 agrega únicamente balances lazy por sucursal y un ledger append-only para `OPENING` y `ADJUSTMENT`.

## Ownership y tablas

`InventoryItem` sigue siendo Organization-owned (`V16`). `branch_inventory` e `inventory_movements` pertenecen a la combinación Organization + Branch + InventoryItem. Las FKs compuestas a `(id, organization_id)` de Branch, InventoryItem y User impiden referencias cross-tenant.

`branch_inventory` mantiene una fila como máximo por `(branch_id, inventory_item_id)`. No se materializa el producto Branch × InventoryItem: una fila ausente significa quantity `0.000000`, version virtual `0`; las lecturas nunca insertan filas.

`inventory_movements` conserva `quantity_delta`, `resulting_quantity`, tipo, motivo, actor y hora UTC. PostgreSQL bloquea `UPDATE` y `DELETE` mediante trigger. En V17 los únicos tipos son `OPENING` y `ADJUSTMENT`; no hay endpoint genérico de movimientos.

## Cantidades exactas

Persistencia `NUMERIC(19,6)` y backend `BigDecimal`. JSON exige strings decimales sin exponentes; responses usan seis decimales fijos, por ejemplo `"-2.500000"`. Se rechazan escala > 6 y magnitud mayor a `9999999999999.999999`; no hay redondeo ni `float`/`double`.

`UNIT` con `fractionalUnitAllowed=false` solo admite saldos enteros; si es `true`, admite fracciones hasta escala seis. `GRAM` y `MILLILITER` admiten fracciones siempre.

## Casos de uso

- Opening Stock acepta cantidad >= 0 (incluye cero), opcionalmente motivo, y solo si no hay ningún movimiento anterior. Solo existe un opening por Branch + Item.
- Adjustment requiere delta firmado, distinto de cero, expectedVersion y razón trim/no vacía de hasta 255 caracteres.
- Los dos casos exigen Branch e InventoryItem `ACTIVE`. Branch e Item inactivos conservan lecturas históricas; no se permiten nuevas mutaciones.
- Un ajuste manual puede dejar balance negativo para representar una corrección de conteo. Esto no define la política futura de checkout/venta, que permanece abierta.

## Serialización y atomicidad

Cada mutación hace `INSERT ... ON CONFLICT DO NOTHING` para asegurar la fila lazy, después `SELECT ... FOR UPDATE` por Organization + Branch + Item. Se valida `expectedVersion` (virtual 0 antes de la primera mutación y luego JPA `@Version`), se actualiza el balance, se inserta el movimiento y se registra AuditEvent en una única transacción. Error en cualquier paso revierte también la fila recién creada.

Permisos `inventory.stock.read` y `inventory.stock.adjust` son branch-scoped mediante `hasBranchPermission`; grants Organization-wide siguen funcionando como override según el RBAC actual. La migración cataloga permisos sin otorgarlos a roles existentes.

## APIs

- `GET /api/v1/organizations/{organizationId}/branches/{branchId}/inventory-items` — paginado, `q`, `status`, `baseUnit`.
- `GET .../inventory-items/{inventoryItemId}` y `GET .../{inventoryItemId}/movements` — lectura histórica paginada.
- `POST .../{inventoryItemId}/opening-stock` y `POST .../{inventoryItemId}/adjustments` — únicos writers de stock de este slice.

No existen PATCH de saldo, DELETE, reservas, Purchasing, Transfers, recipes, costing ni integración Sales.

## Recipe and modifier configuration — V18 candidate / PR C1

V18 añade `product_inventory_recipes` y `product_inventory_recipe_components`, más `modifier_option_inventory_configs` y `modifier_option_inventory_effects`. Son definiciones propiedad de Organization y no incluyen `branch_id`, saldo ni referencias a `SaleItem`. FKs compuestas fuerzan que Product, ModifierOption e InventoryItem pertenezcan a la misma Organization; constraints fijan una receta por Product, una config por ModifierOption y un componente/efecto por InventoryItem dentro del aggregate.

Recetas activas no pueden estar vacías; cada cantidad de componente es positiva. Los efectos de modifiers son deltas firmados no-cero; una lista vacía significa sin impacto. Ambos usan `NUMERIC(19,6)`, `BigDecimal` y strings decimales en API. Los PUT son replace-all, usan `expectedVersion`, locking y `@Version`; cambios de configuración y un AuditEvent se guardan atómicamente. Reemplazos semánticamente iguales no incrementan versión ni generan auditoría.

El resolver combina la receta activa y los ModifierOptions seleccionados ya validados por el caller, suma exacta por InventoryItem, elimina totales cero y rechaza configuraciones con componentes inactivos, saldo teórico negativo o cantidades inválidas. No consulta Branch stock. Product sin receta y receta INACTIVE son untracked. Las APIs de receta y efectos son organization-scoped mediante `inventory.recipe.read/write`; V18 no auto-grants estos permisos a roles existentes.

La configuración es candidata en PR C1. SaleItem snapshots, Sales/Checkout, deducción/reserva de stock, disponibilidad por stock, Purchasing, Transfers, Costing y UI siguen fuera de alcance.
