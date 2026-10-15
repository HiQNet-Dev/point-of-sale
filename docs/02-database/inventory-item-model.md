# Inventory Item catalog — modelo de datos

**Estado:** implementado en `V16`; la inmutabilidad de `base_unit` se añade en `V24`. No modela cantidades; el contrato de balances y movimientos vive en `inventory-branch-stock-model.md`.

## `inventory_items`

| Columna | Tipo | Reglas |
| --- | --- | --- |
| `id` | UUID PK | Generado por aplicación |
| `organization_id` | UUID NOT NULL | FK → `organizations(id)` |
| `code` | VARCHAR(64) NOT NULL | Slug lowercase, inmutable, único por organización |
| `name` | VARCHAR(120) NOT NULL | Trim, no vacío |
| `description` | VARCHAR(1000) NULL | Trim; vacío normalizado a NULL |
| `base_unit` | VARCHAR(16) NOT NULL | `UNIT`, `GRAM` o `MILLILITER`; CHECK |
| `fractional_unit_allowed` | BOOLEAN NOT NULL DEFAULT false | Metadata para futura validación de cantidades |
| `unit_locked` | BOOLEAN NOT NULL DEFAULT false | Congela `base_unit` una vez que el artículo tiene dependencias (`V24`); persistente y monótono (mapeado `updatable = false`, solo lo activa la sentencia del guard) |
| `status` | VARCHAR(16) NOT NULL DEFAULT `ACTIVE` | `ACTIVE` / `INACTIVE`; CHECK |
| `version` | BIGINT NOT NULL DEFAULT 0 | JPA `@Version`; actualización HTTP requiere `expectedVersion` |
| `created_at`, `updated_at` | TIMESTAMPTZ NOT NULL | UTC, `Instant` |

Constraints principales:

- `UNIQUE (organization_id, code)` — PostgreSQL resuelve también carreras concurrentes.
- FK a Organization y superkey `UNIQUE (id, organization_id)` para futuras FKs same-org.
- CHECK del slug, estados, unidades y texto no vacío para name/description.
- Índice `(organization_id, name)` para listing ordenado y filtrado por tenant.

## Propiedad y semántica

`InventoryItem` pertenece a Organization y no contiene `branchId`, quantity ni stock. La misma definición podrá utilizarse en distintas Branches; stock físico será un aggregate branch-scoped posterior.

`code` no se puede cambiar. `name`, description, unidad, flag de fracción y status sí pueden cambiar en esta fase. `UNIT` con `fractionalUnitAllowed=false` representa unidades discretas; con `true`, futura cantidad fraccionaria. `V16` solo almacena la flag; los casos de uso de stock de `V17` aplican la regla a `UNIT`.

`baseUnit` solo puede corregirse mientras el item **no** tenga hechos dependientes. `V24` añade `unit_locked` (default `false`) y lo marca una vez que existe cualquiera de estas dependencias: balance de sucursal (`branch_inventory`), movimiento (`inventory_movements`), componente de receta (`product_inventory_recipe_components`), efecto de modificador (`modifier_option_inventory_effects`), consumo de venta (`sale_item_inventory_components`) o partida de orden de compra (`purchase_order_lines`). El backfill de `V24` marca los items ya referenciados.

El marcador es **persistente**: aunque la dependencia se elimine (reemplazo de receta/efectos, partida en borrador borrada), el artículo sigue bloqueado porque ya fue utilizado. Los artículos sin dependencias mantienen su unidad mutable y una solicitud con la misma unidad es un no-op idempotente. La coordinación contra carreras TOCTOU es de aplicación: todo escritor de dependencias bloquea la fila del artículo (`SELECT ... FOR UPDATE`) antes de leer `base_unit` o de insertar la dependencia, y el cambio de unidad usa el mismo bloqueo (ver ADR-0018). Las unidades de display y conversiones no se modelan.

`INACTIVE` conserva la definición y sigue siendo legible. En V17 bloquea opening/adjustment nuevos pero no las lecturas históricas. No hay hard delete.

## Autorización y auditoría

`inventory.item.read` y `inventory.item.write` son permisos organization-scope. `V16` agrega el catálogo de permisos sin modificar `role_permissions` existente. Bootstrap nuevo asigna todos los permisos del catálogo vigente.

Crear emite `inventory.item.created`; cambios efectivos emiten `inventory.item.updated` con `changedFields`, ambos dentro de la transacción del item. Updates no-op y lecturas no generan AuditEvent.

## Fuera de esta fundación

`V16` does not create balances or quantities. `V17` adds branch-scoped balances, an append-only ledger, opening and manual adjustments. V18 adds organization-owned Product recipes, modifier effects and the per-unit resolver; V19 snapshots resolved values on SaleItems. Atomic checkout stock deduction (`V21`) is implemented; `V24` freezes `base_unit` once the item is used. Purchasing, Transfers and Costing remain deferred.
