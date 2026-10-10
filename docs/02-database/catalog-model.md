# Catalog Foundation — modelo de datos

**Implementado en `V9__create_catalog_foundation.sql`** (PR: Catalog
Foundation). Introduce `Category` y `Product`, propiedad de la **organización**
(catálogo = organización, operación = sucursal). Complementa
`docs/02-database/domain-model.md`, `docs/02-database/organization-branch-model.md`
y `docs/04-security/rbac.md`.

## Propósito

El catálogo comercial (categorías y productos con precio base) se define a
**nivel de organización**: una sucursal *consume* el catálogo, no lo posee.
Las variantes, ingredientes, recetas, precios por sucursal y promociones quedan
explícitamente **fuera de alcance** de esta fundación.

## Modelo

```text
categories
----------
id              UUID     PK
organization_id UUID     -> organizations(id)
code            VARCHAR(64)   slug normalizado (trim+lowercase), inmutable
name            VARCHAR(120)  trim;  CHECK length(trim(name)) > 0
status          VARCHAR(16)   ACTIVE | INACTIVE   (default ACTIVE)
version         BIGINT   @Version (optimistic locking)
created_at / updated_at TIMESTAMPTZ
UNIQUE (organization_id, code)          -- categories_organization_code_unique
CHECK  code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'
UNIQUE (id, organization_id)            -- superkey para FK compuesta

products
--------
id              UUID     PK
organization_id UUID     -> organizations(id)
category_id     UUID     NULL  (opcional)
sku             VARCHAR(64)   identificador de negocio, inmutable, único por org
barcode         VARCHAR(64)   NULL, único por org cuando está presente
name            VARCHAR(120)  trim;  CHECK length(trim(name)) > 0
description     VARCHAR(1000) NULL;  CHECK NULL o no-blank
unit_price      NUMERIC(19,4) NOT NULL CHECK (unit_price >= 0)
status          VARCHAR(16)   ACTIVE | INACTIVE   (default ACTIVE)
version         BIGINT   @Version
created_at / updated_at TIMESTAMPTZ
UNIQUE (organization_id, sku)           -- products_organization_sku_unique
CHECK  sku ~ '^[A-Z0-9._-]{1,64}$'      -- forma canónica MAYÚSCULAS
FK (category_id, organization_id) -> categories (id, organization_id)
```

Índices de lectura: `(organization_id, name)` en ambas tablas y
`(organization_id, category_id)` en `products`. El `barcode` único por
organización es un **índice parcial** (`WHERE barcode IS NOT NULL`):
`products_organization_barcode_unique`.

## Decisiones clave

- **`sku` = identificador de negocio inmutable** (trim + MAYÚSCULAS),
  distinto del UUID. No cambia tras crear el producto.
- **`barcode` opcional**: se guarda como texto para preservar ceros a la
  izquierda (sin parseo GS1 ni multi-barcode todavía).
- **`unit_price` = precio base de la organización** (`NUMERIC(19,4)`). La
  **moneda nunca se guarda por producto**: se hereda de
  `Organization.currency`. Precios por sucursal, promociones e impuestos están
  fuera de alcance.
- **Sin DELETE**: la baja es `status INACTIVE`, que conserva la fila y sus
  productos. Inactivar una categoría **no** cascada al status de sus productos.

## Integridad (PostgreSQL es la autoridad)

| Violación | Cómo la impide PostgreSQL |
| --- | --- |
| Producto Org B + Categoría Org A | FK compuesta `(category_id, organization_id) → categories(id, organization_id)` (superkey `categories_id_organization_unique`) |
| Código de categoría duplicado | `UNIQUE (organization_id, code)` |
| SKU duplicado | `UNIQUE (organization_id, sku)` |
| Barcode duplicado (presente) | Índice parcial único `products_organization_barcode_unique` |
| SKU no canónico / code no slug | `CHECK` de formato |

Los pre-checks de Java (`existsByOrganizationIdAndCode/Sku/Barcode`) son UX; la
carrera concurrente la cierra el constraint traducido por
`ConstraintTranslator` (`categories_organization_code_unique` /
`products_organization_sku_unique` / `products_organization_barcode_unique` →
`409 CATEGORY_CODE_ALREADY_EXISTS` / `PRODUCT_SKU_ALREADY_EXISTS` /
`PRODUCT_BARCODE_ALREADY_EXISTS`).

## Relación con categoría

`products.category_id` es **opcional**. Cuando está presente debe pertenecer a
la **misma organización** (FK compuesta) y estar **ACTIVE** al asignarla
(regla de dominio → `409 CATEGORY_INACTIVE`). Una categoría de otra organización
se resuelve como `404 CATEGORY_NOT_FOUND` (sin filtrar existencia cross-org ni
en create ni en el filtro de listado).

En PATCH de producto, `categoryId` es un campo **tri-estado** (`UuidPatchField`
en `shared.api`): omitir la clave = sin cambios; `"categoryId": null` = desasignar;
`"categoryId": "<uuid>"` = asignar.

## Autorización

Catálogo **organization-scoped**: los seis permisos (`category.read/.create/
.update`, `product.read/.create/.update`) sembrados en `V9` (catálogo 19 → 25) y
evaluados con `hasPermission` (organization scope), **nunca**
`hasBranchPermission` (anti privilege-escalation). Sin DELETE:
`DELETE`/`PUT` no se implementan y caen en `denyAll()`.

## Auditoría

Toda mutación crea un `AuditEvent` en la misma transacción (port
`shared.audit.AuditPort`, actor resuelto centralmente):
`category.created` / `category.updated` y `product.created` / `product.updated`
(`entityType` = `category` / `product`). `updated` registra `changedFields`.

## Endpoints

Véase `docs/03-api/api-guidelines.md` (§ Catalog): `POST/GET
/…/categories`, `GET/PATCH /…/categories/{categoryId}`, `POST/GET
/…/products`, `GET/PATCH /…/products/{productId}` (listado de productos con
filtros `?status&categoryId&q`).
