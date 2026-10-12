# Inventory Item catalog — modelo de datos

**Estado:** candidato / PR abierto. Foundation organization-scoped propuesta en `V16`; no incluye balances ni cantidades de stock.

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

`code` no se puede cambiar. `name`, description, unidad, flag de fracción y status sí pueden cambiar en esta fase. `UNIT` con `fractionalUnitAllowed=false` representa unidades discretas; con `true`, futura cantidad fraccionaria. En este PR el flag se almacena, no valida cantidades porque no hay cantidades todavía.

`baseUnit` puede corregirse mientras no existen dependencias históricas. Una migración futura de Recipes/Balances/Movements debe congelarla cuando el item tenga hechos dependientes. Las unidades de display y conversiones no se modelan.

`INACTIVE` conserva la definición y sigue siendo legible. No hay hard delete. No existe todavía una regla de stock/receta asociada al estado.

## Autorización y auditoría

`inventory.item.read` y `inventory.item.write` son permisos organization-scope. `V16` agrega el catálogo de permisos sin modificar `role_permissions` existente. Bootstrap nuevo asigna todos los permisos del catálogo vigente.

Crear emite `inventory.item.created`; cambios efectivos emiten `inventory.item.updated` con `changedFields`, ambos dentro de la transacción del item. Updates no-op y lecturas no generan AuditEvent.

## Fuera de esta fundación

`BranchInventory`, stock on hand, `InventoryMovement`, apertura/ajustes, recipes, modifier effects, cantidades, cantidades negativas, Purchasing y Sales/checkout no se crean por `V16`.
