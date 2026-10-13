# Inventory Item catalog — modelo de datos

**Estado:** implementado en `V16`. No modela cantidades; el contrato de balances y movimientos vive en `inventory-branch-stock-model.md`.

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

`baseUnit` puede corregirse mientras no existen dependencias históricas. Una migración futura de Recipes/Balances/Movements debe congelarla cuando el item tenga hechos dependientes. Las unidades de display y conversiones no se modelan.

`INACTIVE` conserva la definición y sigue siendo legible. En V17 bloquea opening/adjustment nuevos pero no las lecturas históricas. No hay hard delete.

## Autorización y auditoría

`inventory.item.read` y `inventory.item.write` son permisos organization-scope. `V16` agrega el catálogo de permisos sin modificar `role_permissions` existente. Bootstrap nuevo asigna todos los permisos del catálogo vigente.

Crear emite `inventory.item.created`; cambios efectivos emiten `inventory.item.updated` con `changedFields`, ambos dentro de la transacción del item. Updates no-op y lecturas no generan AuditEvent.

## Fuera de esta fundación

`V16` does not create balances or quantities. `V17` adds branch-scoped balances, an append-only ledger, opening and manual adjustments. V18 adds organization-owned Product recipes, modifier effects and the per-unit resolver; V19 snapshots resolved values on SaleItems. Atomic checkout stock deduction (`V21`) is implemented; Purchasing, Transfers and Costing remain deferred.
