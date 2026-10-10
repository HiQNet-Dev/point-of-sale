# Product Modifiers — modelo de datos

**Implementado en `V10__create_product_modifiers.sql`** (PR: Product Modifiers).
Introduce `ModifierGroup`, `ModifierOption` y la relación `Product ↔ ModifierGroup`,
propiedad de la **organización** (el catálogo se define a nivel de organización).
Complementa `docs/02-database/catalog-model.md`,
`docs/02-database/domain-model.md` y `docs/04-security/rbac.md`.

## Propósito

Un **modifier** ("modificador") es una elección opcional u obligatoria que se
ofrece sobre un producto sin crear una nueva línea de catálogo: tipo de leche,
tamaño, extras, etc. Los grupos son **reutilizables** y se adjuntan a muchos
productos (p. ej. "Milk" aplica a todos los cafés), con un orden de presentación.

Fuera de alcance de esta fundación: variantes/SKU hijos (ver frontera más abajo),
precios por opción ligados a venta, disponibilidad por sucursal, recetas y
snapshot de la elección en la venta.

## Modelo

```text
modifier_groups
---------------
id              UUID     PK
organization_id UUID     -> organizations(id)
code            VARCHAR(64)   slug normalizado (trim+lowercase), inmutable
name            VARCHAR(120)  trim;  CHECK length(trim(name)) > 0
min_selections  INTEGER  NOT NULL DEFAULT 0   CHECK min_selections >= 0
max_selections  INTEGER  NOT NULL DEFAULT 1   CHECK max_selections >= 1
status          VARCHAR(16)   ACTIVE | INACTIVE   (default ACTIVE)
version         BIGINT   @Version (optimistic locking)
created_at / updated_at TIMESTAMPTZ
UNIQUE (organization_id, code)          -- modifier_groups_organization_code_unique
CHECK  min_selections <= max_selections -- modifier_groups_selection_bounds_check
CHECK  code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'
UNIQUE (id, organization_id)            -- superkey para FK compuesta

modifier_options
----------------
id                UUID     PK
organization_id   UUID     -> organizations(id)
modifier_group_id UUID
code              VARCHAR(64)   slug normalizado, inmutable, único dentro del grupo
name              VARCHAR(120)  trim;  CHECK length(trim(name)) > 0
price_delta       NUMERIC(19,4) NOT NULL   -- CON SIGNO (+, 0, -)
display_order     INTEGER  NOT NULL DEFAULT 0   CHECK display_order >= 0
status            VARCHAR(16)   ACTIVE | INACTIVE   (default ACTIVE)
version           BIGINT   @Version
created_at / updated_at TIMESTAMPTZ
UNIQUE (modifier_group_id, code)        -- modifier_options_group_code_unique
CHECK  price_delta = round(price_delta, 4)
FK (modifier_group_id, organization_id) -> modifier_groups (id, organization_id)
UNIQUE (id, organization_id)            -- superkey

product_modifier_groups
-----------------------
organization_id   UUID     (parte de la PK)
product_id        UUID     (parte de la PK)
modifier_group_id UUID
display_order     INTEGER  NOT NULL   -- posición en la lista del producto
created_at        TIMESTAMPTZ
PK (product_id, modifier_group_id)      -- product_modifier_groups_pkey
UNIQUE (product_id, modifier_group_id)  -- product_modifier_groups_product_group_unique
FK (product_id, organization_id)        -> products (id, organization_id)
FK (modifier_group_id, organization_id) -> modifier_groups (id, organization_id)
```

`V10` también añade la superkey `products_id_organization_unique UNIQUE (id,
organization_id)` para permitir las FK compuestas de `product_modifier_groups`.

Índices de lectura: `(organization_id, name)` en `modifier_groups`,
`(organization_id, modifier_group_id, display_order)` en `modifier_options` y
`(product_id)` en `product_modifier_groups`.

## Decisiones clave

- **`code` inmutable** por scope: slug único por organización en grupos, y único
  **dentro del grupo** en opciones (dos grupos distintos pueden tener la opción
  `large`). No cambia tras crear.
- **`name` no único** (dos grupos "Extras" son válidos); solo `code` identifica.
- **Selección derivada, no persistida**: de `min_selections`/`max_selections` se
  derivan `required` (`min > 0`), `singleChoice` (`max == 1`) y `multipleChoice`
  (`max > 1`). No se guardan `required`/`multiple`/`selectionType` (evita estado
  contradictorio).
- **`price_delta` es un ajuste CON SIGNO** (`NUMERIC(19,4)`): `+` recargo, `0`
  sin cambio, `-` descuento. La moneda nunca se guarda por opción: se hereda de
  `Organization.currency`.
- **`status` sin DELETE**: la baja es `status INACTIVE`; no hay cascada de
  desactivación grupo→opciones ni grupo→productos.
- **Orden explícito**: `display_order` en la opción (dentro del grupo) y en la
  relación producto↔grupo (posición en la lista del producto). La posición en el
  `PUT` de asignación ES el `display_order` resultante.

## Integridad (PostgreSQL es la autoridad)

| Violación | Cómo la impide PostgreSQL |
| --- | --- |
| Opción Org B + Grupo Org A | FK compuesta `(modifier_group_id, organization_id) → modifier_groups(id, organization_id)` |
| Asignación producto Org B + Grupo Org A | FK compuestas `(product_id, organization_id)` y `(modifier_group_id, organization_id)` |
| Código de grupo duplicado (por org) | `UNIQUE (organization_id, code)` |
| Código de opción duplicado (dentro del grupo) | `UNIQUE (modifier_group_id, code)` |
| Grupo con `max_selections < 1` o `min > max` | `CHECK` de bounds |
| Opción con `display_order < 0` | `CHECK` |
| `code` no slug | `CHECK` de formato |
| Grupo repetido para un producto | `UNIQUE (product_id, modifier_group_id)` |

Los pre-checks de Java (`existsByOrganizationIdAndCode` /
`existsByModifierGroupIdAndCode`) son UX; la carrera concurrente la cierra el
constraint traducido por `ConstraintTranslator`
(`modifier_groups_organization_code_unique` / `modifier_options_group_code_unique`
→ `409 MODIFIER_GROUP_CODE_ALREADY_EXISTS` /
`MODIFIER_OPTION_CODE_ALREADY_EXISTS`).

## Asignación a un producto (replace atómico)

`PUT /…/products/{productId}/modifier-groups` **reemplaza** la lista ordenada
completa (`modifierGroupIds`); la posición en la lista es el `display_order`.
Semántica:

- Lista `[]` válida → limpia la configuración.
- Duplicados en la lista → `400 VALIDATION_ERROR`.
- Grupo inexistente o de otra organización → `404 MODIFIER_GROUP_NOT_FOUND`.
- **La versión del `Product` es la frontera de concurrencia**: `expectedVersion`
  requerido; mismatch → `409 CONCURRENT_MODIFICATION`; un reemplazo efectivo
  incrementa `Product.version`. Un reemplazo **idéntico** (misma lista en el
  mismo orden) es **no-op**: no muta, no incrementa versión ni emite auditoría.

## Autorización

Grupos y opciones son **organization-scoped** (`modifier.group.*`,
`modifier.option.*`); la asignación producto↔grupo usa `product.read` (lectura) y
`product.modifiers.manage` (reemplazo). Los siete permisos se siembran en `V10`
(catálogo 25 → 32) y se evalúan con `hasPermission` (organization scope),
**nunca** `hasBranchPermission` (anti privilege-escalation). Sin DELETE:
`DELETE`/`PUT` no se implementan y caen en `denyAll()`.

## Auditoría

Toda mutación crea un `AuditEvent` en la misma transacción (port
`shared.audit.AuditPort`, `Propagation.MANDATORY`):
`modifier.group.created` / `modifier.group.updated` (`entityType` =
`modifier_group`), `modifier.option.created` / `modifier.option.updated`
(`entityType` = `modifier_option`) y `product.modifier-groups.replaced`
(`entityType` = `product`). `updated` registra `changedFields`; el reemplazo
registra `modifierGroupCount`. Un reemplazo no-op no emite evento.

## Endpoints

Véase `docs/03-api/api-guidelines.md` (§ Product Modifiers): `POST/GET
/…/modifier-groups`, `GET/PATCH /…/modifier-groups/{groupId}`, `POST/GET
/…/modifier-groups/{groupId}/options`, `GET/PATCH
/…/modifier-groups/{groupId}/options/{optionId}`, `GET/PUT
/…/products/{productId}/modifier-groups`.

## Frontera con variantes (futuro)

Un **modifier** NO es una **variante**. La variante (futuro `ProductVariant`,
SKU hijo con su propio precio/stock) sigue fuera de alcance y no se implementa
aquí. La elección de modifiers que un cliente concreto marca en una venta se
congelará como **snapshot histórico** en el futuro módulo de ventas (todavía no
existe), de modo que el precio/aplicabilidad se congela en el momento de la
venta y no depende de cambios posteriores del catálogo.
